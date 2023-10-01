use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_sessions::Session;
use tracing::info;

use super::{
    error::AuthError,
    oidc::{AuthenticationContext, UserClaims},
};
use crate::app_state::AppState;

pub async fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", get(handle_auth_login))
        .route("/logout", get(handle_auth_logout))
        .route("/callback", get(handle_auth_callback))
}

#[derive(serde::Deserialize)]
struct LoginQuery {
    next: Option<String>,
}

async fn handle_auth_login(
    query: Query<LoginQuery>,
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    let user = session.get::<UserClaims>("user").unwrap_or(None);

    if let Some(user) = user {
        if session.active() {
            return AuthError::AlreadyAuthenticated.into_response();
        }
    }

    if let Some(next_url) = &query.next {
        session.insert("next_url", next_url).unwrap();
    }

    let context = state.auth_client.authorize().await.unwrap();
    session.insert("auth_context", &context).unwrap();

    Redirect::to(context.auth_url.as_str()).into_response()
}

#[derive(Serialize, Deserialize)]
struct ConfirmLoginQuery {
    state: String,
    code: String,
}

async fn handle_auth_callback(
    State(state): State<Arc<AppState>>,
    query: Query<ConfirmLoginQuery>,
    session: Session,
) -> impl IntoResponse {
    let context = session
        .get::<AuthenticationContext>("auth_context")
        .unwrap_or(None);
    if let Some(context) = context {
        session.remove::<AuthenticationContext>("auth_context").ok();
        let claims = state
            .auth_client
            .exchange_code(context, query.code.clone(), query.state.clone())
            .await
            .unwrap();

        session.insert("user", claims).unwrap();

        let next = session.get::<String>("next_url").unwrap_or(None);
        if let Some(next) = next {
            info!("Redirecting to {}", next.as_str());
            session.remove::<String>("next_url").ok();
            Redirect::to(next.as_str()).into_response()
        } else {
            Redirect::to("/").into_response()
        }
    } else {
        AuthError::InvalidTokenExchange.into_response()
    }
}

async fn handle_auth_logout(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    let user = session.remove::<UserClaims>("user").unwrap_or(None);

    "Logged out".into_response()
}
