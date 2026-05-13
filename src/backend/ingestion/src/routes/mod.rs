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
        // Minidump upload endpoint — token identifies the product
        .route("/minidump/{token}/upload", post(MinidumpApi::upload))
        // Health check endpoints (also exposed under /minidump/ for ingress reachability)
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
        .route("/minidump/live", get(health::live))
        .route("/minidump/ready", get(health::ready))
}
