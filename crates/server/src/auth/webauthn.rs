use super::error::AuthError;
use crate::{
    app_state::AppState,
    entity::{
        self,
        prelude::{Credential, User},
    },
};
use app::auth::AuthenticatedUser;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
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

    let user_query = User::find()
        .filter(entity::user::Column::Username.eq(&username))
        .one(&state.db)
        .await?;
    let user_unique_id = get_user_unique_id(user_query, &session).await?;

    let exclude_credentials = Credential::find()
        .filter(entity::credential::Column::UserId.eq(user_unique_id))
        .all(&state.db)
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

    let user = User::find()
        .filter(entity::user::Column::Username.eq(&registration_state.username))
        .one(&state.db)
        .await?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&reg, &registration_state.passkey_registration)?;

    if user.is_none() {
        let user = entity::user::ActiveModel {
            id: Set(registration_state.user_unique_id),
            username: Set(registration_state.username),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            last_authenticated: Set(None),
        };
        user.insert(&state.db).await?;
    }

    let cred = entity::credential::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(registration_state.user_unique_id),
        name: Set("name".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        last_used: Set(Utc::now().naive_utc()),
        data: Set(serde_json::to_value(&passkey)?),
    };
    cred.insert(&state.db).await?;

    Ok(StatusCode::OK)
}

pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, AuthError> {
    session.remove_value("auth_state").await?;

    let user_unique_id = User::find()
        .filter(entity::user::Column::Username.eq(&username))
        .one(&state.db)
        .await?
        .map(|record| record.id)
        .ok_or(AuthError::UserNotFound)?;

    let allow_credentials = Credential::find()
        .filter(entity::credential::Column::UserId.eq(user_unique_id))
        .all(&state.db)
        .await?
        .iter()
        .map(|record| serde_json::from_value::<Passkey>(record.data.clone()))
        .collect::<Result<Vec<Passkey>, _>>()?;

    // TODO AuthError::UserHasNoCredentials

    let (request_challenge_response, passkey_authentication) = state
        .webauthn
        .start_passkey_authentication(allow_credentials.as_slice())?;

    session
        .insert(
            "authentication_state",
            (user_unique_id, passkey_authentication),
        )
        .await?;
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

    update_passkeys(user_unique_id, &state.db, authentication_result).await?;

    let user = User::find()
        .filter(entity::user::Column::Id.eq(user_unique_id))
        .one(&state.db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let authenticated_user = AuthenticatedUser::new(user);
    session
        .insert("authenticated_user", authenticated_user)
        .await?;
    Ok(StatusCode::OK)
}

async fn get_user_unique_id(
    user_query: Option<entity::user::Model>,
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

async fn update_passkeys(
    user_unique_id: Uuid,
    db: &DatabaseConnection,
    auth_result: AuthenticationResult,
) -> Result<(), AuthError> {
    let credentials = Credential::find()
        .filter(entity::credential::Column::UserId.eq(user_unique_id))
        .all(db)
        .await
        .map_err(AuthError::DatabaseError)?;
    for cred in credentials {
        let mut passkey = serde_json::from_value::<Passkey>(cred.data.clone())?;
        let updated = passkey.update_credential(&auth_result);
        if let Some(updated) = updated {
            if updated {
                let mut cred: entity::credential::ActiveModel = cred.into();
                cred.data = Set(serde_json::to_value(&passkey)?);
                cred.update(db).await?;
            }
        }
    }
    Ok(())
}
