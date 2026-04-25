use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Redirect, Response},
    routing::post,
};
use common::AuthenticatedUser;
use tower_sessions::Session;

use crate::{AppState, error::{AppError, AppResult}};

const AUTHENTICATED_USER_SESSION_KEY: &str = "authenticated_user";
const ORIGINAL_USER_SESSION_KEY: &str = "original_user";

const COOKIE_MAX_AGE: u32 = 60 * 60 * 24 * 30;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/impersonate/{user_id}", post(start_impersonation))
        .route("/auth/impersonate/stop", post(stop_impersonation))
}

/// Start impersonating `user_id`.
/// Only real admins (not already impersonating) may do this.
async fn start_impersonation(
    State(state): State<AppState>,
    session: Session,
    Path(user_id): Path<String>,
) -> AppResult<Response> {
    let current = session
        .get::<AuthenticatedUser>(AUTHENTICATED_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::forbidden)?;

    if !current.is_admin {
        return Err(AppError::forbidden());
    }

    // Prevent chaining one impersonation on top of another.
    if session
        .get::<AuthenticatedUser>(ORIGINAL_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .is_some()
    {
        return Err(AppError::failure("Already impersonating — stop first"));
    }

    if current.id == user_id {
        return Err(AppError::failure("Cannot impersonate yourself"));
    }

    let target = repos::user::UserRepo::get_by_id(&state.repo.db, &user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let target_auth = AuthenticatedUser::new(target.id.clone(), target.username, target.is_admin);

    // Swap the effective user in the session; preserve the real admin.
    session
        .insert(ORIGINAL_USER_SESSION_KEY, current.clone())
        .await
        .map_err(AppError::internal)?;
    session
        .insert(AUTHENTICATED_USER_SESSION_KEY, target_auth)
        .await
        .map_err(AppError::internal)?;

    // gr_uid drives SvelteKit's user resolution; gr_real_uid signals the banner.
    let uid_cookie = format!(
        "gr_uid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        target.id, COOKIE_MAX_AGE
    );
    let real_uid_cookie = format!(
        "gr_real_uid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        current.id, COOKIE_MAX_AGE
    );

    let mut response = Redirect::to("/").into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&uid_cookie).map_err(AppError::internal)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&real_uid_cookie).map_err(AppError::internal)?,
    );
    Ok(response)
}

/// Restore the original admin session; clear impersonation.
async fn stop_impersonation(session: Session) -> AppResult<Response> {
    let original = session
        .get::<AuthenticatedUser>(ORIGINAL_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::failure("Not currently impersonating"))?;

    session
        .insert(AUTHENTICATED_USER_SESSION_KEY, original.clone())
        .await
        .map_err(AppError::internal)?;
    session
        .remove::<AuthenticatedUser>(ORIGINAL_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?;

    let uid_cookie = format!(
        "gr_uid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        original.id, COOKIE_MAX_AGE
    );
    let clear_real_uid = "gr_real_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    let mut response = Redirect::to("/").into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&uid_cookie).map_err(AppError::internal)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(clear_real_uid).map_err(AppError::internal)?,
    );
    Ok(response)
}
