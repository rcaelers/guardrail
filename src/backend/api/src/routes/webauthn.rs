use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tower_sessions::Session;
use tracing::error;
use webauthn_rs::prelude::*;

use crate::{error::ApiError, state::AppState};
use common::AuthenticatedUser;
use data::{credentials::NewCredential, user::User};
use repos::{credentials::CredentialsRepo, user::UserRepo};

#[derive(Debug, Serialize, Deserialize)]
struct RegistrationState {
    pub username: String,
    pub user_unique_id: uuid::Uuid,
    pub passkey_registration: PasskeyRegistration,
}

impl RegistrationState {
    fn new(
        username: String,
        user_unique_id: uuid::Uuid,
        passkey_registration: PasskeyRegistration,
    ) -> Self {
        RegistrationState {
            username,
            user_unique_id,
            passkey_registration,
        }
    }
}

pub async fn start_register(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    session.remove_value("passkey_registration_state").await?;

    let db = &state.repo.db;

    let user_query = UserRepo::get_by_name(db, &username).await?;
    let user_unique_id = get_user_unique_id(user_query, &session).await?;

    let exclude_credentials = CredentialsRepo::get_all_by_user_id(db, user_unique_id)
        .await?
        .iter()
        .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
        .collect::<Result<Vec<Passkey>, _>>()
        .map_err(|e| {
            error!("failed to deserialize passkey: {:?}", e);
            ApiError::InternalFailure()
        })?
        .iter()
        .map(|passkey| passkey.cred_id().clone())
        .collect::<Vec<_>>();

    let (creation_challenge_response, passkey_registration) =
        state.webauthn.start_passkey_registration(
            user_unique_id,
            &username,
            &username,
            Some(exclude_credentials),
        )?;

    session
        .insert(
            "passkey_registration_state",
            RegistrationState::new(username, user_unique_id, passkey_registration),
        )
        .await?;

    Ok(Json(creation_challenge_response))
}

pub async fn finish_register(
    State(state): State<AppState>,
    session: Session,
    Json(reg): Json<RegisterPublicKeyCredential>,
) -> Result<impl IntoResponse, ApiError> {
    let registration_state = session
        .get::<RegistrationState>("passkey_registration_state")
        .await?
        .ok_or(ApiError::CorruptSession)?;
    session.remove_value("passkey_registration_state").await?;

    let db = &state.repo.db;

    let user = UserRepo::get_by_name(db, registration_state.username.as_str()).await?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&reg, &registration_state.passkey_registration)?;

    if user.is_none() {
        UserRepo::create_with_id(
            db,
            registration_state.user_unique_id,
            registration_state.username.as_str(),
        )
        .await?;
    }

    CredentialsRepo::create(
        db,
        NewCredential {
            user_id: registration_state.user_unique_id,
            data: serde_json::to_value(&passkey).map_err(|e| {
                error!("failed to serialize passkey: {:?}", e);
                ApiError::InternalFailure()
            })?,
        },
    )
    .await?;

    Ok(StatusCode::OK)
}

pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    session.remove_value("auth_state").await?;

    let db = &state.repo.db;

    let user_unique_id = UserRepo::get_by_name(db, username.as_str())
        .await?
        .map(|record| record.id)
        .ok_or(ApiError::UserNotFound(username))?;

    let allow_credentials = CredentialsRepo::get_all_by_user_id(db, user_unique_id)
        .await?
        .iter()
        .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
        .collect::<Result<Vec<Passkey>, _>>()
        .map_err(|e| {
            error!("failed to deserialize passkey: {:?}", e);
            ApiError::InternalFailure()
        })?;

    // TODO ApiError::UserHasNoCredentials

    let (request_challenge_response, passkey_authentication) = state
        .webauthn
        .start_passkey_authentication(allow_credentials.as_slice())?;

    session
        .insert("authentication_state", (user_unique_id, passkey_authentication))
        .await?;

    Ok(Json(request_challenge_response))
}

pub async fn finish_authentication(
    State(state): State<AppState>,
    session: Session,
    Json(auth): Json<PublicKeyCredential>,
) -> Result<impl IntoResponse, ApiError> {
    let (user_unique_id, auth_state): (Uuid, PasskeyAuthentication) = session
        .get("authentication_state")
        .await?
        .ok_or(ApiError::CorruptSession)?;
    session.remove_value("authentication_state").await?;

    let authentication_result = state
        .webauthn
        .finish_passkey_authentication(&auth, &auth_state)?;

    let db = &state.repo.db;

    update_passkeys(db, user_unique_id, authentication_result).await?;

    let user = UserRepo::get_by_id(db, user_unique_id)
        .await?
        .ok_or(ApiError::CorruptSession)?;

    let authenticated_user = AuthenticatedUser::new(user.id, user.username, user.is_admin);
    session
        .insert("authenticated_user", authenticated_user)
        .await?;

    Ok(StatusCode::OK)
}

async fn get_user_unique_id(
    user_query: Option<User>,
    session: &Session,
) -> Result<uuid::Uuid, ApiError> {
    if let Some(user) = user_query {
        let authenticated_user = session
            .get::<AuthenticatedUser>("authenticated_user")
            .await?;
        if let Some(current_user) = authenticated_user
            && current_user.id == user.id
        {
            return Ok(user.id);
        }
        return Err(ApiError::UserAlreadyExists(user.username));
    }
    Ok(Uuid::new_v4())
}

pub async fn update_passkeys(
    db: &Surreal<Any>,
    user_unique_id: Uuid,
    auth_result: AuthenticationResult,
) -> Result<(), ApiError> {
    let credentials = CredentialsRepo::get_all_by_user_id(db, user_unique_id).await?;
    for cred in credentials {
        let mut passkey = serde_json::from_value::<Passkey>(cred.data.clone()).map_err(|e| {
            error!("failed to deserialize passkey: {:?}", e);
            ApiError::InternalFailure()
        })?;
        let updated = passkey.update_credential(&auth_result);
        if let Some(updated) = updated
            && updated
        {
            CredentialsRepo::update_data(
                db,
                cred.id,
                serde_json::to_value(&passkey).map_err(|e| {
                    error!("failed to serialize passkey: {:?}", e);
                    ApiError::InternalFailure()
                })?,
            )
            .await?;
        }
    }
    Ok(())
}
