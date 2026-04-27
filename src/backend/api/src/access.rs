// Centralized auth guards for the backend API server.
//
// Two token types are accepted where noted:
//   1. Guardrail API token — custom base64url format; verified against the DB.
//   2. JWT bearer — a signed JWT issued by /api/auth/jwt; verified with the
//      configured EdDSA public key.
//
// No session support — backend/api is machine-to-machine only.

use axum::http::{HeaderMap, header};
use common::{
    settings::Settings,
    token::{decode_api_token, verify_api_secret},
};
use data::api_token::ApiToken;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use repos::api_token::ApiTokenRepo;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::warn;
use uuid::Uuid;

use crate::error::ApiError;

// -----------------------------------------------------------------------
// JWT claims
// -----------------------------------------------------------------------

/// Claims embedded in a JWT issued by /api/auth/jwt.
///
/// Defined here so both the token-generation handler and the auth guards
/// share a single definition without a circular module dependency.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub username: String,
    pub user_id: Option<String>,
    pub is_admin: bool,
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub ac: String,
    pub ns: String,
    pub db: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// SurrealDB access method name referenced in JWT claims.
pub const SURREAL_ACCESS_METHOD: &str = "guardrail_api";

// -----------------------------------------------------------------------
// Principal
// -----------------------------------------------------------------------

/// Who is making the request.
#[derive(Clone, Debug)]
pub enum Principal {
    ApiToken(ApiToken),
    Jwt(JwtClaims),
}

impl Principal {
    /// True when the principal has global admin rights.
    ///
    /// For API tokens: a global token (no product restriction) is admin.
    /// For JWTs: the `is_admin` claim determines this.
    pub fn is_admin(&self) -> bool {
        match self {
            Principal::ApiToken(t) => t.product_id.is_none(),
            Principal::Jwt(c) => c.is_admin,
        }
    }
}

// -----------------------------------------------------------------------
// Guards
// -----------------------------------------------------------------------

/// Require a Guardrail API token carrying the given entitlement.
///
/// Checks `Authorization: Bearer/Token` first; falls back to
/// `api_key_fallback` if provided (e.g. from a `?api_key=` query param).
///
/// JWT bearer tokens are rejected — use for machine endpoints where only
/// a typed API token makes sense (symbol upload, JWT generation).
pub async fn require_entitlement(
    headers: &HeaderMap,
    api_key_fallback: Option<&str>,
    db: &Surreal<Any>,
    entitlement: &str,
) -> Result<ApiToken, ApiError> {
    let token_str = extract_bearer_from_headers(headers)
        .or(api_key_fallback)
        .ok_or_else(|| ApiError::InvalidToken("missing API token".into()))?;

    let (token_id, token_secret) = decode_api_token(token_str)
        .map_err(|_| ApiError::InvalidToken("invalid API token".into()))?;

    let api_token = load_and_verify(db, token_id, &token_secret).await?;

    if !api_token.has_entitlement(entitlement) {
        return Err(ApiError::Forbidden("insufficient permissions".into()));
    }

    Ok(api_token)
}

/// Require admin-level access.
///
/// Accepted credentials:
/// - A Guardrail API token with no product restriction (global token).
/// - A JWT with `is_admin: true`.
pub async fn require_admin(
    headers: &HeaderMap,
    db: &Surreal<Any>,
    settings: &Settings,
) -> Result<Principal, ApiError> {
    let token_str = extract_bearer_from_headers(headers)
        .ok_or_else(|| ApiError::InvalidToken("missing API token".into()))?;

    if is_jwt(token_str) {
        let claims = verify_jwt(token_str, settings)?;
        if !claims.is_admin {
            return Err(ApiError::Forbidden("admin access required".into()));
        }
        return Ok(Principal::Jwt(claims));
    }

    let (token_id, token_secret) = decode_api_token(token_str)
        .map_err(|_| ApiError::InvalidToken("invalid API token".into()))?;

    let api_token = load_and_verify(db, token_id, &token_secret).await?;

    if api_token.product_id.is_some() {
        return Err(ApiError::Forbidden("global API token or admin JWT required".into()));
    }

    Ok(Principal::ApiToken(api_token))
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

pub fn extract_bearer_from_headers(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("Token "))
        .map(str::trim)
}

/// JWT tokens have a `header.payload.signature` structure (exactly two dots).
/// Custom API tokens are a single base64url blob with no dots.
fn is_jwt(s: &str) -> bool {
    s.split('.').count() == 3
}

fn verify_jwt(token_str: &str, settings: &Settings) -> Result<JwtClaims, ApiError> {
    let public_key = &settings.auth.jwk.public_key;
    let key =
        DecodingKey::from_ed_pem(public_key.as_bytes()).map_err(|_| ApiError::InternalFailure())?;
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&["guardrail"]);
    decode::<JwtClaims>(token_str, &key, &validation)
        .map(|d| d.claims)
        .map_err(|e| {
            warn!("JWT verification failed: {e}");
            ApiError::InvalidToken("invalid JWT".into())
        })
}

async fn load_and_verify(
    db: &Surreal<Any>,
    token_id: Uuid,
    token_secret: &[u8],
) -> Result<ApiToken, ApiError> {
    let api_token = ApiTokenRepo::get_by_token_id(db, token_id)
        .await
        .map_err(|_| ApiError::InternalFailure())?
        .ok_or_else(|| ApiError::InvalidToken("invalid API token".into()))?;

    let verified = verify_api_secret(token_secret, &api_token.token_hash)
        .map_err(|_| ApiError::InvalidToken("invalid API token".into()))?;

    if !verified {
        return Err(ApiError::InvalidToken("invalid API token".into()));
    }
    if !api_token.is_valid() {
        return Err(ApiError::InvalidToken("API token is expired or inactive".into()));
    }

    if let Err(err) = ApiTokenRepo::update_last_used(db, &api_token.id).await {
        warn!("failed to update token last_used_at: {err}");
    }

    Ok(api_token)
}
