use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_sessions::extractors::{ReadableSession, WritableSession};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, ops::Not};

use crate::app_state::AppState;
use super::{error::AuthError, oidc::UserClaims};

pub async fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", get(handle_auth_login))
        .route("/callback", get(handle_auth_callback))
}

async fn handle_auth_login(
    State(state): State<Arc<AppState>>,
    session: ReadableSession,
) -> impl IntoResponse {
    let user = session.get::<UserClaims>("user");
    if let Some(user) = user {
        if session.is_expired().not() {
            return AuthError::AlreadyAuthenticated.into_response()
        }
    }
    let url = state
        .auth_client
        .authorize()
        .await
        .unwrap()
        .as_str()
        .to_string();
    Redirect::to(url.as_str()).into_response()
}

#[derive(Serialize, Deserialize)]
struct ConfirmLoginQuery {
    state: String,
    code: String,
}

async fn handle_auth_callback(
    State(state): State<Arc<AppState>>,
    query: Query<ConfirmLoginQuery>,
    mut session: WritableSession,
) -> impl IntoResponse {
    let claims = state
        .auth_client
        .exchange_code(query.code.clone(), query.state.clone())
        .await
        .unwrap();

    session.insert("user", &claims).unwrap();

    Redirect::to("/").into_response()
}
