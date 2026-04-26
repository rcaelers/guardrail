use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderValue, header::SET_COOKIE},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use common::AuthenticatedUser;
use serde::Deserialize;
use serde_json::Value;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::AuthSession,
    error::{AppError, AppResult},
    invite, oidc,
    templates::HomeTemplate,
    webauthn,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home))
        .route("/auth/login", get(oidc::login_start))
        .route("/auth/login/start", get(oidc::login_start))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/auth/logout", post(logout))
        .route("/auth/register_start/{username}", post(webauthn::start_register))
        .route("/auth/register_finish", post(webauthn::finish_register))
        .route("/auth/authenticate_start/{username}", post(webauthn::start_authentication))
        .route("/auth/authenticate_finish", post(webauthn::finish_authentication))
        .route("/auth/real-user", get(get_real_user))
        .merge(invite::web_router())
}

pub fn render(template: impl askama::Template) -> AppResult<axum::response::Html<String>> {
    template
        .render()
        .map(axum::response::Html)
        .map_err(AppError::internal)
}

#[derive(Debug, Deserialize)]
struct HomeQuery {
    next: Option<String>,
    error: Option<String>,
}

async fn home(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<HomeQuery>,
) -> AppResult<Html<String>> {
    let auth = auth_session(&session).await;
    let next = oidc::sanitize_next(query.next.as_deref());
    let error = query.error.unwrap_or_default();
    let has_error = !error.is_empty();
    let oidc_enabled = state.settings.auth.oidc.is_some();
    let self_service_url = state
        .settings
        .auth
        .oidc
        .as_ref()
        .map(|o| o.self_service_url.clone())
        .unwrap_or_default();
    render(HomeTemplate {
        title: "Guardrail",
        app_name: state.settings.auth.name.as_str(),
        auth,
        error,
        has_error,
        login_url: oidc::login_start_path(Some(next.as_str())),
        oidc_enabled,
        self_service_url,
    })
}

async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    let mut response = Redirect::to("/").into_response();
    // Clear the SvelteKit-facing cookie alongside the tower-session.
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_static("gr_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response
}

async fn auth_session(session: &Session) -> AuthSession {
    let user = session
        .get::<AuthenticatedUser>(crate::access::SESSION_KEY)
        .await
        .unwrap_or(None);
    AuthSession { user }
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
