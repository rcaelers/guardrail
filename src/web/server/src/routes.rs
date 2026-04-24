use askama::Template;
use axum::{
    Router,
    body::to_bytes,
    extract::{Query, Request, State},
    http::{StatusCode, HeaderValue, header::SET_COOKIE},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{any, get, post},
};
use common::AuthenticatedUser;
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::AuthSession,
    error::{AppError, AppResult},
    oidc,
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
        .fallback(any(dev_proxy))
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
        .get::<AuthenticatedUser>("authenticated_user")
        .await
        .unwrap_or(None);
    AuthSession { user }
}

fn render(template: impl Template) -> AppResult<Html<String>> {
    template.render().map(Html).map_err(AppError::internal)
}

/// Reverse-proxy fallback: forwards unmatched requests to the SvelteKit dev
/// server when `web_server.dev_proxy_url` is configured. In production this
/// field is absent and the handler returns 404.
pub async fn dev_proxy(State(state): State<AppState>, req: Request) -> Response {
    let Some(base) = state.settings.web_server.dev_proxy_url.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let (parts, body) = req.into_parts();
    let pq = parts.uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let target = format!("{}{}", base.trim_end_matches('/'), pq);

    let bytes = to_bytes(body, 32 * 1024 * 1024).await.unwrap_or_default();

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);

    let mut rb = state.http_client.request(method, &target);
    for (k, v) in &parts.headers {
        if !matches!(k.as_str(), "host" | "connection" | "keep-alive"
                     | "transfer-encoding" | "te" | "trailer" | "upgrade") {
            rb = rb.header(k.as_str(), v);
        }
    }
    if !bytes.is_empty() {
        rb = rb.body(bytes);
    }

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("dev proxy {target}: {e}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = axum::http::Response::builder().status(status);
    for (k, v) in resp.headers() {
        if !matches!(k.as_str(), "connection" | "keep-alive" | "transfer-encoding") {
            builder = builder.header(k, v);
        }
    }
    let body_bytes = resp.bytes().await.unwrap_or_default();
    builder
        .body(axum::body::Body::from(body_bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
