use axum::{extract::State, http::HeaderMap, response::IntoResponse};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use tracing::{error, info};

use crate::{
    access::{self, JwtClaims, SURREAL_ACCESS_METHOD},
    error::ApiError,
    state::AppState,
};
use repos::user::UserRepo;

fn user_record_id(username: &str, user_id: Option<&str>) -> String {
    let id = user_id.unwrap_or(username);
    if id.contains(':') {
        id.to_string()
    } else {
        format!("users:{id}")
    }
}

pub async fn generate_jwt_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let api_token = access::require_entitlement(&headers, None, &state.repo.db, "token").await?;
    let settings = state.settings.clone();
    let expiration = Utc::now() + Duration::minutes(settings.auth.jwk.token_validity_in_minutes);
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
        iss: settings.auth.id.clone(),
        aud: "guardrail".to_string(),
        exp: expiration_timestamp,
        iat: Utc::now().timestamp(),
        ac: SURREAL_ACCESS_METHOD.to_string(),
        ns: settings.database.namespace.clone(),
        db: settings.database.database.clone(),
        id: Some(user_record_id(&username, user_id.as_deref())),
    };

    let private_key = &settings.auth.jwk.private_key;

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

/// Generate a raw token/hash pair for use when creating a new API token.
/// Requires a global API token or an admin JWT — this endpoint is admin-only.
pub async fn generate_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    access::require_admin(&headers, &state.repo.db, &state.settings).await?;
    let (token_id, token, token_hash) =
        common::token::generate_api_token().map_err(|_| ApiError::InternalFailure())?;
    Ok(axum::Json(
        serde_json::json!({ "token_id": token_id, "token": token, "token_hash": token_hash }),
    ))
}
