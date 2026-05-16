mod health;
mod symbols;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;
use symbols::SymbolsApi;

pub async fn routes(_app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/symbols/{token}/upload", post(SymbolsApi::upload))
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
