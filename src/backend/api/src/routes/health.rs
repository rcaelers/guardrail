use axum::{extract::State, http::StatusCode};
use tracing::error;

use crate::state::AppState;

pub async fn live() -> StatusCode {
    StatusCode::OK
}

pub async fn ready(State(state): State<AppState>) -> StatusCode {
    let mut conn = match state.repo.acquire_admin().await {
        Ok(conn) => conn,
        Err(err) => {
            error!("Health check failed to get database connection: {}", err);
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    };

    if sqlx::query("SELECT 1").execute(&mut *conn).await.is_ok() {
        return StatusCode::OK;
    }
    StatusCode::SERVICE_UNAVAILABLE
}
