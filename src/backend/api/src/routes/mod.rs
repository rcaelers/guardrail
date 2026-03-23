mod health;
mod symbols;
mod token;
mod webauthn;

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
        // WebAuthn authentication endpoints
        .route("/auth/register_start/{username}", post(webauthn::start_register))
        .route("/auth/register_finish", post(webauthn::finish_register))
        .route("/auth/authenticate_start/{username}", post(webauthn::start_authentication))
        .route("/auth/authenticate_finish", post(webauthn::finish_authentication))
        // Health check endpoints
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
