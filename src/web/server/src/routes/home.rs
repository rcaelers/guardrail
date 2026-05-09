use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState, auth_user::AuthenticatedUser, error::AppResult, oidc, templates::HomeTemplate,
};

use super::render;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(home))
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
    let auth = session
        .get::<AuthenticatedUser>(crate::access::SESSION_KEY)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    let next = oidc::sanitize_next(query.next.as_deref());
    let error = query.error.unwrap_or_default();
    let has_error = !error.is_empty();
    let self_service_url = state
        .settings
        .auth
        .oidc
        .as_ref()
        .and_then(|o| o.self_service_url.clone())
        .unwrap_or_default();
    render(HomeTemplate {
        title: "Guardrail",
        app_name: state.settings.auth.name.as_str(),
        auth,
        error,
        has_error,
        login_url: oidc::login_start_path(Some(next.as_str())),
        self_service_url,
    })
}
