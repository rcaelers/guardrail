use axum::{extract::State, http::StatusCode};
use tracing::error;

use crate::state::AppState;

pub async fn live() -> StatusCode {
    StatusCode::OK
}

pub async fn ready(State(state): State<AppState>) -> StatusCode {
    if state.product_cache.is_healthy().await {
        StatusCode::OK
    } else {
        error!("Health check failed: Valkey is not reachable");
        StatusCode::SERVICE_UNAVAILABLE
    }
}
