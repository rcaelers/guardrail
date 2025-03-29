use axum::{Router, routing::post};

use super::{
    api_token::{ApiTokenLayer, RequiredEntitlement},
    minidump::MinidumpApi,
    symbols::SymbolsApi,
    token::generate_jwt_token,
};
use crate::app_state::AppState;

pub async fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        // Symbol upload endpoint
        .route(
            "/symbols/upload",
            post(SymbolsApi::upload)
                .layer(ApiTokenLayer::new(app_state.clone(), RequiredEntitlement::SymbolUpload)),
        )
        // Minidump upload endpoint
        .route(
            "/minidump/upload",
            post(MinidumpApi::upload)
                .layer(ApiTokenLayer::new(app_state.clone(), RequiredEntitlement::MinidumpUpload)),
        )
        // JWT token generation endpoint
        .route(
            "/auth/token",
            post(generate_jwt_token)
                .layer(ApiTokenLayer::new(app_state.clone(), RequiredEntitlement::Token)),
        )
        // WebAuthn authentication endpoints
        .route("/auth/register_start/{username}", post(super::webauthn::start_register))
        .route("/auth/register_finish", post(super::webauthn::finish_register))
        .route("/auth/authenticate_start/{username}", post(super::webauthn::start_authentication))
        .route("/auth/authenticate_finish", post(super::webauthn::finish_authentication))
}
