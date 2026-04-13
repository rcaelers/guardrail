use askama::Template;
use axum::{
    Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use common::AuthenticatedUser;
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::AuthSession,
    error::{AppError, AppResult},
    templates::{HomeTemplate, LoginTemplate},
    webauthn,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home))
        .route("/auth/login", get(login_page))
        .route("/auth/logout", post(logout))
        .route("/auth/register_start/{username}", post(webauthn::start_register))
        .route("/auth/register_finish", post(webauthn::finish_register))
        .route(
            "/auth/authenticate_start/{username}",
            post(webauthn::start_authentication),
        )
        .route(
            "/auth/authenticate_finish",
            post(webauthn::finish_authentication),
        )
}

async fn home(State(state): State<AppState>, session: Session) -> AppResult<Html<String>> {
    let auth = auth_session(&session).await;
    render(HomeTemplate {
        title: "Guardrail",
        app_name: state.settings.auth.name.as_str(),
        auth,
    })
}

#[derive(Debug, Deserialize)]
struct LoginQuery {
    next: Option<String>,
}

async fn login_page(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<LoginQuery>,
) -> AppResult<Html<String>> {
    let auth = auth_session(&session).await;
    render(LoginTemplate {
        title: "Sign in",
        app_name: state.settings.auth.name.as_str(),
        auth,
        next: query.next.unwrap_or_else(|| "/".to_string()),
    })
}

async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    Redirect::to("/")
}

async fn auth_session(session: &Session) -> AuthSession {
    let user = session
        .get::<AuthenticatedUser>("authenticated_user")
        .await
        .unwrap_or(None);
    AuthSession { user }
}

fn render(template: impl Template) -> AppResult<Html<String>> {
    template.render().map(Html).map_err(AppError::internal)
}
