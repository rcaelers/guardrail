use axum::http::{HeaderMap, header};
use common::token::{decode_api_token, verify_api_secret};
use data::api_token::ApiToken;
use repos::api_token::ApiTokenRepo;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::warn;
use uuid::Uuid;

use crate::error::ApiError;

/// Require a Guardrail API token carrying the given entitlement.
///
/// Checks `Authorization: Bearer/Token` first; falls back to
/// `api_key_fallback` if provided (e.g. from a `?api_key=` query param).
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

/// Require a global (non-product-scoped) API token — grants admin access.
pub async fn require_admin(headers: &HeaderMap, db: &Surreal<Any>) -> Result<ApiToken, ApiError> {
    let token_str = extract_bearer_from_headers(headers)
        .ok_or_else(|| ApiError::InvalidToken("missing API token".into()))?;

    let (token_id, token_secret) = decode_api_token(token_str)
        .map_err(|_| ApiError::InvalidToken("invalid API token".into()))?;

    let api_token = load_and_verify(db, token_id, &token_secret).await?;

    if api_token.product_id.is_some() {
        return Err(ApiError::Forbidden("global API token required".into()));
    }

    Ok(api_token)
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

    #[tokio::test]
    async fn require_admin_rejects_product_scoped_api_tokens() {
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
            require_admin(&headers, &db).await,
            Err(ApiError::Forbidden(message))
                if message == "global API token required"
        ));
    }
}
