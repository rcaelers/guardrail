use askama::Template;
use axum::{response::IntoResponse, routing, Router};
use std::sync::Arc;
use tower_sessions::Session;

use crate::{
    app_state::AppState,
    auth::{error::AuthError, user::AuthenticatedUser},
};

pub async fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/", routing::get(index))
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    pub current_user: Option<AuthenticatedUser>,
}

async fn index(session: Session) -> Result<impl IntoResponse, AuthError> {
    let current_user = session
        .get::<AuthenticatedUser>("authenticated_user")
        .await?;

    Ok(IndexPage { current_user }.into_response())
}
