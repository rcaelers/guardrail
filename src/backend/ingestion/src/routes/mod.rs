mod health;
mod minidump;

use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{get, post},
};

use crate::rate_limit::rate_limit;
use crate::state::AppState;
use minidump::MinidumpApi;

pub async fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        // Minidump upload endpoint — token identifies the product. The upload
        // route (and only it) is wrapped with the Valkey-backed rate limiter.
        .route(
            "/minidump/{token}/upload",
            post(MinidumpApi::upload)
                .layer(from_fn_with_state(app_state.clone(), rate_limit)),
        )
        // Health check endpoints (also exposed under /minidump/ for ingress reachability)
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
        .route("/minidump/live", get(health::live))
        .route("/minidump/ready", get(health::ready))
}
