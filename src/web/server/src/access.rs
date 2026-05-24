// Centralized authentication and authorization guards for the web server.
//
// All guards that hit the database use the root `Surreal<Any>` connection so
// they are not subject to row-level security.  Handlers may still call
// `user_db()` for data queries that should be RLS-scoped.
//
// The web server is session-only: every browser-facing endpoint requires a
// valid tower session.  Bearer/API tokens are accepted only on the separate
// ingestion/API server (`src/backend/api`).  The token helpers below
// (`require_entitlement`, `require_session_or_entitlement`) are kept for
// potential future use but are not wired into any web-server route.

use axum::http::{HeaderMap, header};
use common::token::{decode_api_token, verify_api_secret};
use tracing::{debug, warn};

use crate::auth_user::{AuthenticatedUser, User};
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
    #[allow(dead_code)]
    pub fn is_admin(&self) -> bool {
        match self {
            Principal::User(u) => u.is_admin(),
            Principal::Token(t) => t.product_id.is_none(),
        }
    }

    /// A stable identifier for logging / audit trails.
    #[allow(dead_code)]
    pub fn display_id(&self) -> String {
        match self {
            Principal::User(u) => u.active().id.clone(),
            Principal::Token(t) => format!("api-token:{}", t.id),
        }
    }
}

// --------------------------------------------------------------------
// Session guards (browser-only flows)
// --------------------------------------------------------------------

/// Require a valid session.  Returns the authenticated user or `Forbidden`.
///
/// Use for flows that can only come from a browser (OIDC callbacks, impersonation).
pub async fn require_session(session: &Session) -> AppResult<AuthenticatedUser> {
    session
        .get::<AuthenticatedUser>(SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            debug!("require_session: no authenticated session");
            AppError::forbidden()
        })
}

/// Require a valid session *and* `is_admin = true`.
///
/// Use for session-only admin actions such as impersonation that must never
/// accept a token.
#[allow(dead_code)]
pub async fn require_session_admin(
    session: &Session,
    db: &Surreal<Any>,
) -> AppResult<AuthenticatedUser> {
    let user = require_current_session_user(session, db).await?;
    if !user.is_admin() {
        warn!(user_id = %user.active().id, "require_session_admin: access denied, not an admin");
        return Err(AppError::forbidden());
    }
    debug!(user_id = %user.active().id, "require_session_admin: access granted");
    Ok(user)
}

// --------------------------------------------------------------------
// Token guard (machine-to-machine, no session fallback)
// --------------------------------------------------------------------

/// Require a Bearer token with the given entitlement.  No session fallback.
///
/// Use for endpoints consumed by automated systems (crash submission, symbol
/// upload) that never involve a browser session.
#[allow(dead_code)]
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

/// Require an admin session user (`is_admin = true`).
///
/// Session-only: Bearer tokens are rejected.  Use for web-server endpoints
/// that must only be called from an authenticated browser session.
pub async fn require_admin(
    session: &Session,
    _headers: &HeaderMap,
    db: &Surreal<Any>,
) -> AppResult<Principal> {
    let user = require_current_session_user(session, db).await?;
    if !user.is_admin() {
        warn!(user_id = %user.active().id, "require_admin: access denied, not an admin");
        return Err(AppError::forbidden());
    }
    debug!(user_id = %user.active().id, "require_admin: access granted");
    Ok(Principal::User(user))
}

/// Require maintainer-level access for a specific product.
///
/// Session-only: Bearer tokens are rejected.  Browser users must have the
/// maintainer role for `product_id`; global admins are always allowed.
pub async fn require_product_maintainer(
    session: &Session,
    _headers: &HeaderMap,
    db: &Surreal<Any>,
    product_id: &str,
) -> AppResult<Principal> {
    let user = require_current_session_user(session, db).await?;
    if user.is_admin() {
        debug!(user_id = %user.active().id, product_id, "require_product_maintainer: access granted (admin)");
        return Ok(Principal::User(user));
    }
    let maintained = get_maintained_product_ids(db, &user.active().id).await?;
    if !maintained.contains(&product_id.to_string()) {
        warn!(user_id = %user.active().id, product_id, "require_product_maintainer: access denied, not a maintainer");
        return Err(AppError::forbidden());
    }
    debug!(user_id = %user.active().id, product_id, "require_product_maintainer: access granted");
    Ok(Principal::User(user))
}

/// Require a browser user to have at least `required_role` on `product_id`.
///
/// Admin users satisfy every product role. The hierarchy is:
/// `maintainer > readwrite > readonly`.
pub async fn require_session_product_role(
    session: &Session,
    db: &Surreal<Any>,
    product_id: &str,
    required_role: &str,
) -> AppResult<AuthenticatedUser> {
    let user = require_current_session_user(session, db).await?;
    if user.is_admin() {
        debug!(user_id = %user.active().id, product_id, required_role, "require_session_product_role: access granted (admin)");
        return Ok(user);
    }

    let uid = repos::record_key(&user.active().id);
    let pid = repos::record_key(product_id);
    let mut result = db
        .query(
            "SELECT VALUE role FROM user_access
             WHERE user_id = type::record('users', $uid)
               AND product_id = type::record('products', $pid)",
        )
        .bind(("uid", uid))
        .bind(("pid", pid))
        .await
        .map_err(AppError::internal)?;

    let roles: Vec<String> = result.take(0).map_err(AppError::internal)?;
    if roles
        .iter()
        .any(|role| product_role_satisfies(role, required_role))
    {
        debug!(user_id = %user.active().id, product_id, required_role, "require_session_product_role: access granted");
        return Ok(user);
    }

    warn!(user_id = %user.active().id, product_id, required_role, ?roles, "require_session_product_role: access denied, insufficient role");
    Err(AppError::forbidden())
}

/// Require a browser user to be reading their own user-scoped data, or an
/// admin browser user to be reading any user's data.
///
/// Session-only: Bearer tokens are rejected.
pub async fn require_user_or_admin(
    session: &Session,
    _headers: &HeaderMap,
    db: &Surreal<Any>,
    user_id: &str,
) -> AppResult<Principal> {
    let user = require_current_session_user(session, db).await?;
    if user.is_admin() || repos::record_key(&user.active().id) == repos::record_key(user_id) {
        debug!(user_id = %user.active().id, target_user_id = user_id, "require_user_or_admin: access granted");
        return Ok(Principal::User(user));
    }
    warn!(user_id = %user.active().id, target_user_id = user_id, "require_user_or_admin: access denied");
    Err(AppError::forbidden())
}

/// Require authentication via session OR a Bearer token with the given
/// entitlement.
///
/// The token path is taken only when an `Authorization` header is present;
/// otherwise a session is required.  Use for endpoints that accept both
/// human users and automated API callers (e.g. invitation creation).
#[allow(dead_code)]
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

fn product_role_satisfies(actual: &str, required: &str) -> bool {
    let rank = |role: &str| match role {
        "readonly" => Some(1),
        "readwrite" => Some(2),
        "maintainer" => Some(3),
        _ => None,
    };
    match (rank(actual), rank(required)) {
        (Some(actual), Some(required)) => actual >= required,
        _ => false,
    }
}

async fn require_current_session_user(
    session: &Session,
    db: &Surreal<Any>,
) -> AppResult<AuthenticatedUser> {
    let session_auth = session
        .get::<AuthenticatedUser>(SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::forbidden)?;

    let active_id = session_auth
        .user
        .as_ref()
        .ok_or_else(AppError::forbidden)?
        .id
        .clone();

    let user = fetch_user(db, &active_id)
        .await?
        .ok_or_else(|| {
            warn!(user_id = %active_id, "session user no longer exists in database");
            AppError::forbidden()
        })?;

    Ok(AuthenticatedUser {
        user: Some(user),
        real_user: session_auth.real_user,
        id_token: session_auth.id_token,
    })
}

async fn fetch_user(db: &Surreal<Any>, user_id: &str) -> AppResult<Option<User>> {
    let uid = repos::record_key(user_id);
    let mut result = db
        .query(
            "SELECT meta::id(id) AS id, username, is_admin, avatar \
             FROM ONLY type::record('users', $uid)",
        )
        .bind(("uid", uid))
        .await
        .map_err(AppError::internal)?;

    let row: Option<serde_json::Value> = result.take(0).map_err(AppError::internal)?;
    let Some(row) = row.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    Ok(Some(User {
        id: row
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(user_id)
            .to_string(),
        name: row
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("anonymous")
            .to_string(),
        is_admin: row
            .get("is_admin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        avatar: row
            .get("avatar")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
    }))
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

    if let Some(ent) = entitlement
        && !token.has_entitlement(ent)
    {
        return Err(AppError::forbidden());
    }

    touch_token_last_used(db, &token.id).await;

    debug!(token_id = %token.id, "API token verified");
    Ok(token)
}

async fn touch_token_last_used(db: &Surreal<Any>, token_id: &str) {
    if let Err(err) = ApiTokenRepo::update_last_used(db, token_id).await {
        warn_token_last_used_update_failed(err);
    }
}

fn warn_token_last_used_update_failed(err: impl std::fmt::Display) {
    tracing::warn!("failed to update token last_used_at: {err}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn api_token(id: &str, product_id: Option<&str>) -> ApiToken {
        ApiToken {
            id: id.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            description: "test".to_string(),
            token_id: Uuid::new_v4(),
            token_hash: "hash".to_string(),
            product_id: product_id.map(str::to_string),
            user_id: None,
            entitlements: vec![],
            last_used_at: None,
            expires_at: None,
            is_active: true,
        }
    }

    #[test]
    fn principal_admin_and_display_id_cover_user_and_token_variants() {
        let user = AuthenticatedUser::authenticated(User {
            id: "user-1".to_string(),
            name: "User".to_string(),
            is_admin: true,
            avatar: None,
        });
        let user_principal = Principal::User(user);
        assert!(user_principal.is_admin());
        assert_eq!(user_principal.display_id(), "user-1");

        let global_token = Principal::Token(api_token("token-1", None));
        assert!(global_token.is_admin());
        assert_eq!(global_token.display_id(), "api-token:token-1");

        let scoped_token = Principal::Token(api_token("token-2", Some("product-1")));
        assert!(!scoped_token.is_admin());
        assert_eq!(scoped_token.display_id(), "api-token:token-2");
    }

    #[tokio::test]
    async fn fetch_user_returns_none_for_missing_record() {
        testware::setup::TestSetup::init();
        let db = testware::setup::TestSetup::create_db().await;

        assert!(fetch_user(&db, "missing").await.unwrap().is_none());
    }
}
