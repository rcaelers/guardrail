use axum::{response::IntoResponse, routing::get, Router};
use std::sync::Arc;

use crate::app_state::AppState;

use super::error::WebError;

pub async fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/test", get(handle_get_test))
}

async fn handle_get_test() -> impl IntoResponse {
    WebError::Failure.into_response()
}
