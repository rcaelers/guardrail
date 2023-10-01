use axum::{response::IntoResponse, routing::get, Router};
use std::sync::Arc;

use crate::{app_state::AppState, auth::layer::AuthLayer};

pub async fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/test", get(handle_get_test))
        .layer(AuthLayer::new(Arc::clone(&state.auth_client)))
        .route("/", get(handle_get_root))
}

async fn handle_get_test() -> impl IntoResponse {
    "Test: Hello, world!"
}

async fn handle_get_root() -> impl IntoResponse {
    "Root: Hello, world!"
}
