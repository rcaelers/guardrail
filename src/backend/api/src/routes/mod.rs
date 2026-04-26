mod health;
mod symbols;
mod token;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;
use symbols::SymbolsApi;

pub async fn routes(_app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/symbols/upload", post(SymbolsApi::upload))
        .route("/auth/jwt", post(token::generate_jwt_token))
        .route("/auth/token", post(token::generate_token))
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
