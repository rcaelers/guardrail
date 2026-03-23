mod health;
mod minidump;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;
use minidump::MinidumpApi;

pub async fn routes(_app_state: AppState) -> Router<AppState> {
    Router::new()
        // Minidump upload endpoint
        .route("/minidump/upload", post(MinidumpApi::upload))
        // Health check endpoints
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
