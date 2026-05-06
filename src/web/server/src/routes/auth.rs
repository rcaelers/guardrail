use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use common::AuthenticatedUser;
use serde_json::Value;
use tower_sessions::Session;

use crate::{
    AppState,
    error::{AppError, AppResult},
    oidc,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(oidc::login_start))
        .route("/auth/login/start", get(oidc::login_start))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/auth/logout", post(logout))
        .route("/auth/real-user", get(get_real_user))
}

async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    let mut response = Redirect::to("/").into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_static("gr_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_static("gr_real_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response
}

/// Returns the real (admin) user's data when impersonation is active.
/// Reads from the session (trusted server-side state) and queries root DB.
/// 404 if not currently impersonating.
async fn get_real_user(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    let original = session
        .get::<AuthenticatedUser>("original_user")
        .await
        .map_err(AppError::internal)?;

    let Some(original) = original else {
        return Err(AppError::not_found("not impersonating"));
    };

    let mut result = state
        .repo
        .db
        .query(
            "SELECT meta::id(id) AS id, email, name, avatar, \
             is_admin AS isAdmin, created_at AS joinedAt \
             FROM ONLY type::record('users', $id)",
        )
        .bind(("id", original.id.clone()))
        .await
        .map_err(AppError::internal)?;

    let rows: Vec<Value> = result.take(0).map_err(AppError::internal)?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| AppError::not_found("real user not found"))
}
