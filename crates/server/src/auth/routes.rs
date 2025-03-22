use super::webauthn::{
    finish_authentication, finish_register, start_authentication, start_register,
};
use axum::{Router, routing};

use crate::app_state::AppState;

pub async fn routes() -> Router<AppState> {
    Router::new()
        .route("/register_start/{username}", routing::post(start_register))
        .route("/register_finish", routing::post(finish_register))
        .route(
            "/authenticate_start/{username}",
            routing::post(start_authentication),
        )
        .route("/authenticate_finish", routing::post(finish_authentication))
}
