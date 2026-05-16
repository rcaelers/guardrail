// Centralized auth guards for the backend API server.
//
// Two token types are accepted where noted:
//   1. Guardrail API token — custom base64url format; verified against the DB.
//   2. JWT bearer — a signed JWT issued by /api/auth/jwt; verified with the
//      configured EdDSA public key.
//
// No session support — backend/api is machine-to-machine only.

use crate::settings::Settings;
use axum::http::{HeaderMap, header};
use common::token::{decode_api_token, verify_api_secret};
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{EncodingKey, Header, encode};

    fn settings() -> Settings {
        let mut settings = crate::settings::Settings::test_default();
        settings.database.namespace = "testns".to_string();
        settings.database.database = "testdb".to_string();
        settings
    }

    fn jwt(settings: &Settings, is_admin: bool) -> String {
        let claims = JwtClaims {
            username: "alice".to_string(),
            user_id: Some("users:alice".to_string()),
            is_admin,
            sub: "alice".to_string(),
            iss: "guardrail".to_string(),
            aud: "guardrail".to_string(),
            exp: (Utc::now() + Duration::minutes(10)).timestamp(),
            iat: Utc::now().timestamp(),
            ac: SURREAL_ACCESS_METHOD.to_string(),
            ns: settings.database.namespace.clone(),
            db: settings.database.database.clone(),
            id: Some("users:alice".to_string()),
        };
        let key = EncodingKey::from_ed_pem(settings.auth.jwk.private_key.as_bytes()).unwrap();
        encode(&Header::new(Algorithm::EdDSA), &claims, &key).unwrap()
    }

    #[test]
    fn principal_admin_rules_match_token_type() {
        let api_token = ApiToken {
            id: "api_tokens:test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            description: "test".to_string(),
            token_id: Uuid::new_v4(),
            token_hash: "hash".to_string(),
            product_id: None,
            user_id: None,
            entitlements: vec![],
            last_used_at: None,
            expires_at: None,
            is_active: true,
        };
        assert!(Principal::ApiToken(api_token.clone()).is_admin());

        let product_token = ApiToken {
            product_id: Some("products:one".to_string()),
            ..api_token
        };
        assert!(!Principal::ApiToken(product_token).is_admin());

        let mut claims = JwtClaims {
            username: "alice".to_string(),
            user_id: None,
            is_admin: true,
            sub: "alice".to_string(),
            iss: "issuer".to_string(),
            aud: "guardrail".to_string(),
            exp: Utc::now().timestamp(),
            iat: Utc::now().timestamp(),
            ac: SURREAL_ACCESS_METHOD.to_string(),
            ns: "ns".to_string(),
            db: "db".to_string(),
            id: None,
        };
        assert!(Principal::Jwt(claims.clone()).is_admin());
        claims.is_admin = false;
        assert!(!Principal::Jwt(claims).is_admin());
    }

    #[test]
    fn extracts_bearer_and_token_authorization_headers() {
        let mut headers = HeaderMap::new();
        assert_eq!(extract_bearer_from_headers(&headers), None);

        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer abc"));
        assert_eq!(extract_bearer_from_headers(&headers), Some("abc"));

        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Token def"));
        assert_eq!(extract_bearer_from_headers(&headers), Some("def"));

        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic xyz"));
        assert_eq!(extract_bearer_from_headers(&headers), None);
    }

    #[test]
    fn verifies_valid_jwt_and_rejects_invalid_jwt() {
        let settings = settings();
        let token = jwt(&settings, true);

        let claims = verify_jwt(&token, &settings).unwrap();
        assert_eq!(claims.username, "alice");
        assert!(claims.is_admin);

        assert!(matches!(
            verify_jwt("not.a.jwt", &settings),
            Err(ApiError::InvalidToken(message)) if message == "invalid JWT"
        ));
    }

    #[tokio::test]
    async fn require_admin_accepts_admin_jwt_and_rejects_non_admin_jwt() {
        let settings = settings();
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        let mut headers = HeaderMap::new();

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", jwt(&settings, true))).unwrap(),
        );
        let principal = require_admin(&headers, &db, &settings).await.unwrap();
        assert!(matches!(principal, Principal::Jwt(claims) if claims.is_admin));

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", jwt(&settings, false))).unwrap(),
        );
        assert!(matches!(
            require_admin(&headers, &db, &settings).await,
            Err(ApiError::Forbidden(message)) if message == "admin access required"
        ));
    }

    #[tokio::test]
    async fn require_admin_rejects_product_scoped_api_tokens() {
        let settings = settings();
        let db = testware::setup::TestSetup::create_db().await;
        let product =
            testware::create_test_product_with_details(&db, "TestProduct", "description").await;
        let (token, _) =
            testware::create_test_token(&db, "product token", Some(product.id), None, &[]).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );

        assert!(matches!(
            require_admin(&headers, &db, &settings).await,
            Err(ApiError::Forbidden(message))
                if message == "global API token or admin JWT required"
        ));
    }
}
