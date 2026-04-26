// Centralized authentication and authorization guards for the web server.
//
// All guards that hit the database use the root `Surreal<Any>` connection so
// they are not subject to row-level security.  Handlers may still call
// `user_db()` for data queries that should be RLS-scoped.
//
// Three auth paths exist:
//   1. Session — a browser user logged in via WebAuthn or OIDC.
//   2. Bearer token — an API token in the Authorization header.
//   3. Either — session or Bearer depending on what the endpoint accepts.

use axum::http::{HeaderMap, header};
use common::{
    AuthenticatedUser,
    token::{decode_api_token, verify_api_secret},
};
use data::api_token::ApiToken;
use repos::api_token::ApiTokenRepo;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tower_sessions::Session;

use crate::error::{AppError, AppResult};

/// Session key used to store and retrieve the authenticated user.
pub const SESSION_KEY: &str = "authenticated_user";

// --------------------------------------------------------------------
// Principal
// --------------------------------------------------------------------

/// Who is making the request.
#[derive(Clone, Debug)]
pub enum Principal {
    User(AuthenticatedUser),
    Token(ApiToken),
}

impl Principal {
    /// True when the principal has global admin rights.
    ///
    /// For users this means `is_admin = true`.  For tokens this means the
    /// token has no product restriction (it is a global token).
    pub fn is_admin(&self) -> bool {
        match self {
            Principal::User(u) => u.is_admin,
            Principal::Token(t) => t.product_id.is_none(),
        }
    }

    /// A stable identifier for logging / audit trails.
    pub fn display_id(&self) -> String {
        match self {
            Principal::User(u) => u.id.clone(),
            Principal::Token(t) => format!("api-token:{}", t.id),
        }
    }
}

// --------------------------------------------------------------------
// Session guards (browser-only flows)
// --------------------------------------------------------------------

/// Require a valid session.  Returns the authenticated user or `Forbidden`.
///
/// Use for flows that can only come from a browser (WebAuthn, OIDC callbacks,
/// impersonation).
pub async fn require_session(session: &Session) -> AppResult<AuthenticatedUser> {
    session
        .get::<AuthenticatedUser>(SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::forbidden)
}

/// Require a valid session *and* `is_admin = true`.
///
/// Use for session-only admin actions such as impersonation that must never
/// accept a token.
pub async fn require_session_admin(session: &Session) -> AppResult<AuthenticatedUser> {
    let user = require_session(session).await?;
    if !user.is_admin {
        return Err(AppError::forbidden());
    }
    Ok(user)
}

// --------------------------------------------------------------------
// Token guard (machine-to-machine, no session fallback)
// --------------------------------------------------------------------

/// Require a Bearer token with the given entitlement.  No session fallback.
///
/// Use for endpoints consumed by automated systems (crash submission, symbol
/// upload) that never involve a browser session.
pub async fn require_entitlement(
    headers: &HeaderMap,
    db: &Surreal<Any>,
    entitlement: &str,
) -> AppResult<ApiToken> {
    let token_str = extract_bearer_token(headers).ok_or_else(AppError::forbidden)?;
    verify_and_touch_token(db, token_str, Some(entitlement)).await
}

// --------------------------------------------------------------------
// Mixed guards (session OR token)
// --------------------------------------------------------------------

/// Require an admin principal.
///
/// * Session users must have `is_admin = true`.
/// * Bearer token callers must present a *global* token (no product
///   restriction).
pub async fn require_admin(
    session: &Session,
    headers: &HeaderMap,
    db: &Surreal<Any>,
) -> AppResult<Principal> {
    if has_bearer_token(headers) {
        let token_str = extract_bearer_token(headers).ok_or_else(AppError::forbidden)?;
        let token = verify_and_touch_token(db, token_str, None).await?;
        if token.product_id.is_some() {
            return Err(AppError::forbidden());
        }
        return Ok(Principal::Token(token));
    }
    let user = require_session(session).await?;
    if !user.is_admin {
        return Err(AppError::forbidden());
    }
    Ok(Principal::User(user))
}

/// Require maintainer-level access for a specific product.
///
/// * Session users must have the maintainer role for `product_id` (admins
///   are always allowed).
/// * Bearer token callers must present a token scoped to `product_id` *or* a
///   global token (no product restriction).
pub async fn require_product_maintainer(
    session: &Session,
    headers: &HeaderMap,
    db: &Surreal<Any>,
    product_id: &str,
) -> AppResult<Principal> {
    if has_bearer_token(headers) {
        let token_str = extract_bearer_token(headers).ok_or_else(AppError::forbidden)?;
        let token = verify_and_touch_token(db, token_str, None).await?;
        // Global tokens are accepted; product-scoped tokens must match.
        if let Some(pid) = &token.product_id {
            if pid != product_id {
                return Err(AppError::forbidden());
            }
        }
        return Ok(Principal::Token(token));
    }
    let user = require_session(session).await?;
    if user.is_admin {
        return Ok(Principal::User(user));
    }
    let maintained = get_maintained_product_ids(db, &user.id).await?;
    if !maintained.contains(&product_id.to_string()) {
        return Err(AppError::forbidden());
    }
    Ok(Principal::User(user))
}

/// Require authentication via session OR a Bearer token with the given
/// entitlement.
///
/// The token path is taken only when an `Authorization` header is present;
/// otherwise a session is required.  Use for endpoints that accept both
/// human users and automated API callers (e.g. invitation creation).
pub async fn require_session_or_entitlement(
    session: &Session,
    headers: &HeaderMap,
    db: &Surreal<Any>,
    entitlement: &str,
) -> AppResult<Principal> {
    if has_bearer_token(headers) {
        let token = require_entitlement(headers, db, entitlement).await?;
        return Ok(Principal::Token(token));
    }
    let user = require_session(session).await?;
    Ok(Principal::User(user))
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

/// Returns product IDs (plain UUID / slug strings) where the user has the
/// maintainer role.
pub async fn get_maintained_product_ids(
    db: &Surreal<Any>,
    user_id: &str,
) -> AppResult<Vec<String>> {
    let uid = repos::record_key(user_id);
    let mut result = db
        .query(
            "SELECT meta::id(product_id) as pid FROM user_access
             WHERE user_id = type::record('users', $uid)
               AND role = 'maintainer'",
        )
        .bind(("uid", uid))
        .await
        .map_err(AppError::internal)?;

    let rows: Vec<serde_json::Value> = result.take(0).map_err(AppError::internal)?;
    let ids = rows
        .into_iter()
        .filter_map(|v| v.get("pid").and_then(|p| p.as_str()).map(str::to_owned))
        .collect();
    Ok(ids)
}

/// True when the request carries a `Bearer` or `Token` Authorization header.
pub fn has_bearer_token(headers: &HeaderMap) -> bool {
    extract_bearer_token(headers).is_some()
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("Token "))
        .map(str::trim)
}

/// Decode and verify an API token, update `last_used_at`, and return it.
///
/// When `entitlement` is `Some`, also checks that the token carries that
/// entitlement.
async fn verify_and_touch_token(
    db: &Surreal<Any>,
    token_str: &str,
    entitlement: Option<&str>,
) -> AppResult<ApiToken> {
    let (token_id, token_secret) = decode_api_token(token_str).map_err(|err| {
        tracing::warn!("invalid API token encoding: {err}");
        AppError::forbidden()
    })?;

    let token = ApiTokenRepo::get_by_token_id(db, token_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::forbidden)?;

    let verified = verify_api_secret(&token_secret, &token.token_hash).map_err(|err| {
        tracing::warn!("failed to verify API token: {err}");
        AppError::forbidden()
    })?;

    if !verified || !token.is_valid() {
        return Err(AppError::forbidden());
    }

    if let Some(ent) = entitlement {
        if !token.has_entitlement(ent) {
            return Err(AppError::forbidden());
        }
    }

    if let Err(err) = ApiTokenRepo::update_last_used(db, &token.id).await {
        tracing::warn!("failed to update token last_used_at: {err}");
    }

    Ok(token)
}
