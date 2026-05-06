use axum::{
    Router,
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    routing::post,
};
use common::AuthenticatedUser;
use tower_sessions::Session;

use crate::{
    AppState, access,
    error::{AppError, AppResult},
};

const ORIGINAL_USER_SESSION_KEY: &str = "original_user";

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
    let current = access::require_session_admin(&session, &state.repo.db).await?;

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
        .insert(access::SESSION_KEY, target_auth)
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to("/").into_response())
}

/// Restore the original admin session; clear impersonation.
async fn stop_impersonation(session: Session) -> AppResult<Response> {
    let original = session
        .get::<AuthenticatedUser>(ORIGINAL_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::failure("Not currently impersonating"))?;

    session
        .insert(access::SESSION_KEY, original.clone())
        .await
        .map_err(AppError::internal)?;
    session
        .remove::<AuthenticatedUser>(ORIGINAL_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to("/").into_response())
}
