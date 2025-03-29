use super::error::AuthError;
use crate::app_state::AppState;
use axum::{
    extract::{Extension, State},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use common::settings::settings;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use repos::api_token::ApiToken;
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub username: String, // Username
    pub role: String,     // Role (e.g., "admin")
    pub sub: String,      // Subject (username)
    pub iss: String,      // Issuer
    pub aud: String,      // Audience (product_id if available)
    pub exp: i64,         // Expiration time
    pub iat: i64,         // Issued at time
}

pub async fn generate_jwt_token(
    State(state): State<AppState>,
    Extension(api_token): Extension<ApiToken>,
) -> Result<impl IntoResponse, AuthError> {
    let expiration = Utc::now() + Duration::minutes(settings().auth.jwk.token_validity_in_minutes);
    let expiration_timestamp = expiration.timestamp();

    let mut conn = match state.repo.acquire_admin().await {
        Ok(conn) => conn,
        Err(err) => {
            error!("Failed to get database connection: {}", err);
            return Err(AuthError::Failure);
        }
    };

    let username = if let Some(user_id) = api_token.user_id {
        match repos::user::UserRepo::get_by_id(&mut *conn, user_id).await {
            Ok(Some(user)) => user.username,
            Ok(None) => {
                error!("User not found for API token: {}", api_token.id);
                return Err(AuthError::UserNotFound);
            }
            Err(err) => {
                error!("Failed to retrieve user: {}", err);
                return Err(AuthError::Failure);
            }
        }
    } else {
        info!("API token {} has no associated user, using 'admin'", api_token.id);
        "admin".to_string()
    };

    let claims = JwtClaims {
        username: username.clone(),
        role: "guardrail_apiuser".to_string(),
        sub: username.clone(),
        iss: settings().auth.id.clone(),
        aud: "guardrail".to_string(),
        exp: expiration_timestamp,
        iat: Utc::now().timestamp(),
    };

    let private_key_path = &settings().auth.jwk.private_key;
    let private_key = match fs::read(private_key_path) {
        Ok(key) => key,
        Err(err) => {
            error!("Failed to read private key: {}", err);
            return Err(AuthError::Failure);
        }
    };

    let encoding_key = match EncodingKey::from_ed_pem(&private_key) {
        Ok(key) => key,
        Err(err) => {
            error!("Failed to create encoding key: {}", err);
            return Err(AuthError::Failure);
        }
    };

    let header = Header::new(Algorithm::EdDSA);

    let token = match encode(&header, &claims, &encoding_key) {
        Ok(t) => t,
        Err(err) => {
            error!("Failed to encode JWT token: {}", err);
            return Err(AuthError::Failure);
        }
    };

    info!(
        "Generated JWT token for user: {} using API token: {} ({})",
        username, api_token.id, api_token.description
    );

    Ok(axum::Json(serde_json::json!({ "token": token })))
}
