use super::error::AuthError;
use crate::app_state::AppState;
use app::auth::AuthenticatedUser;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use repos::{
    credentials::CredentialRepo,
    user::{User, UserRepo},
};
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use tower_sessions::Session;
use webauthn_rs::prelude::*;

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
) -> Result<impl IntoResponse, AuthError> {
    session.remove_value("passkey_registration_state").await?;

    let mut tx = state.repo.begin_admin().await?;

    let user_query = UserRepo::get_by_name(&mut *tx, &username).await?;
    let user_unique_id = get_user_unique_id(user_query, &session).await?;

    let exclude_credentials = CredentialRepo::get_all_by_user_id(&mut *tx, user_unique_id)
        .await?
        .iter()
        .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
        .collect::<Result<Vec<Passkey>, _>>()?
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

    tx.commit().await?;
    Ok(Json(creation_challenge_response))
}

pub async fn finish_register(
    State(state): State<AppState>,
    session: Session,
    Json(reg): Json<RegisterPublicKeyCredential>,
) -> Result<impl IntoResponse, AuthError> {
    let registration_state = session
        .get::<RegistrationState>("passkey_registration_state")
        .await?
        .ok_or(AuthError::CorruptSession)?;
    session.remove_value("passkey_registration_state").await?;

    let mut tx = state.repo.begin_admin().await?;

    let user = UserRepo::get_by_name(&mut *tx, registration_state.username.as_str()).await?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&reg, &registration_state.passkey_registration)?;

    if user.is_none() {
        UserRepo::create_with_id(
            &mut *tx,
            registration_state.user_unique_id,
            registration_state.username.as_str(),
        )
        .await?;
    }

    CredentialRepo::create(
        &mut *tx,
        registration_state.user_unique_id,
        serde_json::to_value(&passkey)?,
    )
    .await?;

    tx.commit().await?;
    Ok(StatusCode::OK)
}

pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, AuthError> {
    session.remove_value("auth_state").await?;

    let mut tx = state.repo.begin_admin().await?;

    let user_unique_id = UserRepo::get_by_name(&mut *tx, username.as_str())
        .await?
        .map(|record| record.id)
        .ok_or(AuthError::UserNotFound)?;

    let allow_credentials = CredentialRepo::get_all_by_user_id(&mut *tx, user_unique_id)
        .await?
        .iter()
        .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
        .collect::<Result<Vec<Passkey>, _>>()?;

    // TODO AuthError::UserHasNoCredentials

    let (request_challenge_response, passkey_authentication) = state
        .webauthn
        .start_passkey_authentication(allow_credentials.as_slice())?;

    session
        .insert("authentication_state", (user_unique_id, passkey_authentication))
        .await?;

    tx.commit().await?;
    Ok(Json(request_challenge_response))
}

pub async fn finish_authentication(
    State(state): State<AppState>,
    session: Session,
    Json(auth): Json<PublicKeyCredential>,
) -> Result<impl IntoResponse, AuthError> {
    let (user_unique_id, auth_state): (Uuid, PasskeyAuthentication) = session
        .get("authentication_state")
        .await?
        .ok_or(AuthError::CorruptSession)?;
    session.remove_value("authentication_state").await?;

    let authentication_result = state
        .webauthn
        .finish_passkey_authentication(&auth, &auth_state)?;

    let mut tx = state.repo.begin_admin().await?;

    update_passkeys(&mut *tx, user_unique_id, authentication_result).await?;

    let user = UserRepo::get_by_id(&mut *tx, user_unique_id)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let authenticated_user = AuthenticatedUser::new(user.id, user.username, user.is_admin);
    session
        .insert("authenticated_user", authenticated_user)
        .await?;

    tx.commit().await?;
    Ok(StatusCode::OK)
}

async fn get_user_unique_id(
    user_query: Option<User>,
    session: &Session,
) -> Result<uuid::Uuid, AuthError> {
    if let Some(user) = user_query {
        let authenticated_user = session
            .get::<AuthenticatedUser>("authenticated_user")
            .await?;
        if let Some(current_user) = authenticated_user {
            if current_user.id == user.id {
                return Ok(user.id);
            }
        }
        return Err(AuthError::UserAlreadyExists);
    }
    Ok(Uuid::new_v4())
}

pub async fn update_passkeys<E>(
    tx: &mut E,
    user_unique_id: Uuid,
    auth_result: AuthenticationResult,
) -> Result<(), AuthError>
where
    for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
{
    let credentials = CredentialRepo::get_all_by_user_id(&mut *tx, user_unique_id).await?;
    for cred in credentials {
        let mut passkey = serde_json::from_value::<Passkey>(cred.data.clone())?;
        let updated = passkey.update_credential(&auth_result);
        if let Some(updated) = updated {
            if updated {
                CredentialRepo::update_data(&mut *tx, cred.id, serde_json::to_value(&passkey)?)
                    .await?;
            }
        }
    }
    Ok(())
}
