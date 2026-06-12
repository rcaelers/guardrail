use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
    routing::post,
};
use tower_sessions::Session;

use crate::{
    AppState, access,
    auth_user::{AuthenticatedUser, User},
    error::{AppError, AppResult},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/impersonate/{user_id}", post(start_impersonation))
        .route("/auth/impersonate/stop", post(stop_impersonation))
}

/// The `scheme://authority` part of the configured public base URL.
fn public_origin(base_url: &str) -> Option<String> {
    let base = base_url.trim().trim_end_matches('/').to_ascii_lowercase();
    let after_scheme = base.find("://")? + 3;
    let end = base[after_scheme..]
        .find('/')
        .map_or(base.len(), |i| after_scheme + i);
    Some(base[..end].to_string())
}

/// CSRF guard for these native-form endpoints: `/auth/*` is proxied straight
/// to this server, bypassing the SvelteKit double-submit hook, so on top of
/// the SameSite=Lax session cookie require the browser-set `Origin` (falling
/// back to `Referer`) to match the public base URL.
fn require_same_origin(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let Some(expected) = public_origin(&state.settings.ingress.base_url) else {
        return Err(AppError::internal("ingress base_url has no origin"));
    };
    let ok = if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        origin.trim().trim_end_matches('/').eq_ignore_ascii_case(&expected)
    } else if let Some(referer) = headers.get(header::REFERER).and_then(|v| v.to_str().ok()) {
        let referer = referer.trim().to_ascii_lowercase();
        referer == expected || referer.starts_with(&format!("{expected}/"))
    } else {
        false
    };
    if ok { Ok(()) } else { Err(AppError::forbidden()) }
}

/// Start impersonating `user_id`.
/// Only real admins (not already impersonating) may do this.
async fn start_impersonation(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> AppResult<Response> {
    require_same_origin(&state, &headers)?;
    let current = access::require_session_admin(&session, &state.repo.db).await?;

    if current.is_impersonating() {
        return Err(AppError::failure("Already impersonating — stop first"));
    }

    if current.active().id == user_id {
        return Err(AppError::failure("Cannot impersonate yourself"));
    }

    let target = repos::user::UserRepo::get_by_id(&state.repo.db, &user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let target_auth = AuthenticatedUser {
        user: Some(User {
            id: target.id,
            name: target.username,
            is_admin: target.is_admin,
            avatar: None,
        }),
        real_user: current.user,
        id_token: None,
    };

    session
        .insert(access::SESSION_KEY, target_auth)
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to("/").into_response())
}

/// Restore the original admin session; clear impersonation.
async fn stop_impersonation(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> AppResult<Response> {
    require_same_origin(&state, &headers)?;
    let user = access::require_session(&session).await?;
    let admin = user
        .real_user
        .ok_or_else(|| AppError::failure("Not currently impersonating"))?;

    session
        .insert(access::SESSION_KEY, AuthenticatedUser::authenticated(admin))
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to("/").into_response())
}

#[cfg(test)]
mod tests {
    use super::public_origin;

    #[test]
    fn public_origin_strips_path_and_trailing_slash() {
        assert_eq!(
            public_origin("https://guardrail.example.com").as_deref(),
            Some("https://guardrail.example.com")
        );
        assert_eq!(
            public_origin("https://Guardrail.Example.com/app/").as_deref(),
            Some("https://guardrail.example.com")
        );
        assert_eq!(
            public_origin("http://localhost:8082").as_deref(),
            Some("http://localhost:8082")
        );
        assert_eq!(public_origin("not-a-url"), None);
    }
}
