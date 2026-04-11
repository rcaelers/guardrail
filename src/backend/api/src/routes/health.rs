use axum::{extract::State, http::StatusCode};
use tracing::error;

use crate::state::AppState;

pub async fn live() -> StatusCode {
    StatusCode::OK
}

pub async fn ready(State(state): State<AppState>) -> StatusCode {
    match state.repo.db.health().await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!("Health check failed: {}", err);
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
