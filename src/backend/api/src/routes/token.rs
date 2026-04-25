use axum::{
    extract::{Extension, State},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{error::ApiError, state::AppState};
use data::api_token::ApiToken;
use repos::user::UserRepo;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub username: String,
    pub user_id: Option<String>,
    pub is_admin: bool,
    pub sub: String,
    pub iss: String,  // Issuer
    pub aud: String,  // Audience (product_id if available)
    pub exp: i64,     // Expiration time
    pub iat: i64,     // Issued at time
    // SurrealDB record-access claims (allow this JWT to authenticate with SurrealDB)
    pub ac: String, // Access method name
    pub ns: String, // SurrealDB namespace
    pub db: String, // SurrealDB database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // SurrealDB record id (e.g. "users:uuid")
}

/// The SurrealDB access method name used for JWT-based record authentication.
pub const SURREAL_ACCESS_METHOD: &str = "guardrail_api";

pub async fn generate_jwt_token(
    State(state): State<AppState>,
    Extension(api_token): Extension<ApiToken>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = state.settings.clone();
    let expiration =
        Utc::now() + Duration::minutes(settings.clone().auth.jwk.token_validity_in_minutes);
    let expiration_timestamp = expiration.timestamp();

    let (username, user_id, is_admin) = if let Some(user_id_raw) = api_token.user_id.as_deref() {
        let Some(user) = UserRepo::get_by_id(&state.repo.db, user_id_raw)
            .await
            .map_err(|err| {
                error!("Failed to retrieve user {}: {}", user_id_raw, err);
                ApiError::Failure("invalid API token".to_string())
            })?
        else {
            error!("User not found for API token: {}", api_token.id);
            return Err(ApiError::Failure("invalid API token".to_string()));
        };
        (user.username, Some(user.id), user.is_admin)
    } else {
        info!("API token {} has no associated user, using 'admin'", api_token.id);
        ("admin".to_string(), None, true)
    };

    let claims = JwtClaims {
        username: username.clone(),
        user_id: user_id.clone(),
        is_admin,
        sub: username.clone(),
        iss: settings.clone().auth.id.clone(),
        aud: "guardrail".to_string(),
        exp: expiration_timestamp,
        iat: Utc::now().timestamp(),
        ac: SURREAL_ACCESS_METHOD.to_string(),
        ns: settings.database.namespace.clone(),
        db: settings.database.database.clone(),
        id: user_id.clone(),
    };

    let private_key = &settings.clone().auth.jwk.private_key;

    let encoding_key = match EncodingKey::from_ed_pem(private_key.as_bytes()) {
        Ok(key) => key,
        Err(err) => {
            error!("Failed to create encoding key: {}", err);
            return Err(ApiError::InternalFailure());
        }
    };

    let header = Header::new(Algorithm::EdDSA);

    let token = match encode(&header, &claims, &encoding_key) {
        Ok(t) => t,
        Err(err) => {
            error!("Failed to encode JWT token: {}", err);
            return Err(ApiError::InternalFailure());
        }
    };

    info!(
        "Generated JWT token for user: {} using API token: {} ({})",
        username, api_token.id, api_token.description
    );

    Ok(axum::Json(serde_json::json!({ "token": token })))
}

pub async fn generate_token() -> Result<impl IntoResponse, ApiError> {
    let (token_id, token, token_hash) =
        common::token::generate_api_token().map_err(|_| ApiError::InternalFailure())?;
    Ok(axum::Json(
        serde_json::json!({ "token_id": token_id, "token": token, "token_hash": token_hash
        }),
    ))
}
