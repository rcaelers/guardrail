mod health;
mod symbols;
mod token;

use axum::{
    Router,
    routing::{get, post},
};

use super::api_token::{ApiTokenLayer, RequiredEntitlement};
use crate::state::AppState;
use symbols::SymbolsApi;

pub async fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        // Symbol upload endpoint
        .route(
            "/symbols/upload",
            post(SymbolsApi::upload)
                .layer(ApiTokenLayer::new(app_state.clone(), RequiredEntitlement::SymbolUpload)),
        )
        // JWT token generation endpoint
        .route(
            "/auth/jwt",
            post(token::generate_jwt_token)
                .layer(ApiTokenLayer::new(app_state.clone(), RequiredEntitlement::Token)),
        )
        .route("/auth/token", post(token::generate_token))
        // Health check endpoints
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
