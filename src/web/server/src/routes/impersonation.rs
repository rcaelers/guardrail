use axum::{
    Router,
    extract::{Path, State},
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

/// Start impersonating `user_id`.
/// Only real admins (not already impersonating) may do this.
async fn start_impersonation(
    State(state): State<AppState>,
    session: Session,
    Path(user_id): Path<String>,
) -> AppResult<Response> {
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
async fn stop_impersonation(session: Session) -> AppResult<Response> {
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
