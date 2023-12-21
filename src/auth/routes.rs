use super::{
    error::AuthError,
    user::AuthenticatedUser,
    webauthn::{finish_authentication, finish_register, start_authentication, start_register},
};
use askama::Template;
use askama_axum::IntoResponse;
use axum::{routing, Router};
use std::sync::Arc;
use tower_sessions::Session;

use crate::app_state::AppState;

pub async fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", routing::get(register))
        .route("/login", routing::get(login))
        .route("/logout", routing::get(logout))
        .route("/register_start/:username", routing::post(start_register))
        .route("/register_finish", routing::post(finish_register))
        .route(
            "/authenticate_start/:username",
            routing::post(start_authentication),
        )
        .route("/authenticate_finish", routing::post(finish_authentication))
}

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterPage {
    pub current_user: Option<AuthenticatedUser>,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    pub current_user: Option<AuthenticatedUser>,
}

#[derive(Template)]
#[template(path = "logout.html")]
struct LogoutPage {
    pub current_user: Option<AuthenticatedUser>,
}

async fn register(session: Session) -> Result<impl IntoResponse, AuthError> {
    let current_user = session
        .get::<AuthenticatedUser>("authenticated_user")
        .await
        .unwrap_or(None);
    Ok(RegisterPage { current_user }.into_response())
}

async fn login(session: Session) -> Result<impl IntoResponse, AuthError> {
    let current_user = session
        .get::<AuthenticatedUser>("authenticated_user")
        .await
        .unwrap_or(None);
    Ok(LoginPage { current_user }.into_response())
}

async fn logout(session: Session) -> Result<impl IntoResponse, AuthError> {
    let current_user = session
        .remove::<AuthenticatedUser>("authenticated_user")
        .await
        .unwrap_or(None);
    Ok(LogoutPage { current_user }.into_response())
}
