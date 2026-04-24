use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use common::AuthenticatedUser;
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use crate::{
    AppState,
    error::{AppError, AppResult},
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RegistrationState {
    username: String,
    user_unique_id: uuid::Uuid,
    passkey_registration: PasskeyRegistration,
}

pub async fn start_register(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> AppResult<impl IntoResponse> {
    session
        .remove_value("passkey_registration_state")
        .await
        .map_err(AppError::internal)?;

    let user_query = repos::user::UserRepo::get_by_name(&state.repo.db, &username)
        .await
        .map_err(AppError::internal)?;
    let user_unique_id = get_user_unique_id(user_query, &session).await?;

    let exclude_credentials =
        repos::credentials::CredentialsRepo::get_all_by_user_id(&state.repo.db, user_unique_id)
            .await
            .map_err(AppError::internal)?
            .iter()
            .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::internal)?
            .iter()
            .map(|passkey| passkey.cred_id().clone())
            .collect::<Vec<_>>();

    let (creation_challenge_response, passkey_registration) = state
        .webauthn
        .start_passkey_registration(user_unique_id, &username, &username, Some(exclude_credentials))
        .map_err(AppError::internal)?;

    session
        .insert(
            "passkey_registration_state",
            RegistrationState {
                username,
                user_unique_id,
                passkey_registration,
            },
        )
        .await
        .map_err(AppError::internal)?;

    Ok(axum::Json(creation_challenge_response))
}

pub async fn finish_register(
    State(state): State<AppState>,
    session: Session,
    axum::Json(reg): axum::Json<RegisterPublicKeyCredential>,
) -> AppResult<impl IntoResponse> {
    let registration_state = session
        .get::<RegistrationState>("passkey_registration_state")
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::corrupt_session)?;
    session
        .remove_value("passkey_registration_state")
        .await
        .map_err(AppError::internal)?;

    let user =
        repos::user::UserRepo::get_by_name(&state.repo.db, registration_state.username.as_str())
            .await
            .map_err(AppError::internal)?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&reg, &registration_state.passkey_registration)
        .map_err(AppError::internal)?;

    let user_id = if let Some(user) = user {
        user.id
    } else {
        repos::user::UserRepo::create_with_id(
            &state.repo.db,
            registration_state.user_unique_id.to_string(),
            registration_state.username.as_str(),
        )
        .await
        .map_err(AppError::internal)?
    };

    repos::credentials::CredentialsRepo::create(
        &state.repo.db,
        data::credentials::NewCredential {
            user_id,
            data: serde_json::to_value(&passkey).map_err(AppError::internal)?,
        },
    )
    .await
    .map_err(AppError::internal)?;

    Ok(StatusCode::OK)
}

pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> AppResult<impl IntoResponse> {
    session
        .remove_value("authentication_state")
        .await
        .map_err(AppError::internal)?;

    let user_unique_id = repos::user::UserRepo::get_by_name(&state.repo.db, username.as_str())
        .await
        .map_err(AppError::internal)?
        .map(|record| record.id)
        .ok_or_else(|| AppError::not_found(format!("User {username} not found")))?;

    let allow_credentials =
        repos::credentials::CredentialsRepo::get_all_by_user_id(&state.repo.db, &user_unique_id)
            .await
            .map_err(AppError::internal)?
            .iter()
            .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::internal)?;

    let (request_challenge_response, passkey_authentication) = state
        .webauthn
        .start_passkey_authentication(allow_credentials.as_slice())
        .map_err(AppError::internal)?;

    session
        .insert("authentication_state", (user_unique_id, passkey_authentication))
        .await
        .map_err(AppError::internal)?;

    Ok(axum::Json(request_challenge_response))
}

pub async fn finish_authentication(
    State(state): State<AppState>,
    session: Session,
    axum::Json(auth): axum::Json<PublicKeyCredential>,
) -> AppResult<impl IntoResponse> {
    let (user_unique_id, auth_state): (String, PasskeyAuthentication) = session
        .get("authentication_state")
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::corrupt_session)?;
    session
        .remove_value("authentication_state")
        .await
        .map_err(AppError::internal)?;

    let authentication_result = state
        .webauthn
        .finish_passkey_authentication(&auth, &auth_state)
        .map_err(AppError::internal)?;

    update_passkeys(&state, &user_unique_id, authentication_result).await?;

    let user = repos::user::UserRepo::get_by_id(&state.repo.db, user_unique_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::corrupt_session)?;

    session
        .insert("authenticated_user", AuthenticatedUser::new(user.id, user.username, user.is_admin))
        .await
        .map_err(AppError::internal)?;

    Ok(StatusCode::OK)
}

async fn get_user_unique_id(
    user_query: Option<data::user::User>,
    session: &Session,
) -> AppResult<uuid::Uuid> {
    if let Some(user) = user_query {
        let authenticated_user = session
            .get::<AuthenticatedUser>("authenticated_user")
            .await
            .map_err(AppError::internal)?;
        if let Some(current_user) = authenticated_user
            && current_user.id == user.id
        {
            return Ok(uuid::Uuid::new_v4());
        }
        return Err(AppError::failure(format!("User {} already exists", user.username)));
    }

    Ok(uuid::Uuid::new_v4())
}

async fn update_passkeys(
    state: &AppState,
    user_unique_id: &str,
    auth_result: AuthenticationResult,
) -> AppResult<()> {
    let credentials =
        repos::credentials::CredentialsRepo::get_all_by_user_id(&state.repo.db, user_unique_id)
            .await
            .map_err(AppError::internal)?;

    for cred in credentials {
        let mut passkey =
            serde_json::from_value::<Passkey>(cred.data.clone()).map_err(AppError::internal)?;
        if let Some(updated) = passkey.update_credential(&auth_result)
            && updated
        {
            repos::credentials::CredentialsRepo::update_data(
                &state.repo.db,
                cred.id,
                serde_json::to_value(&passkey).map_err(AppError::internal)?,
            )
            .await
            .map_err(AppError::internal)?;
        }
    }

    Ok(())
}
