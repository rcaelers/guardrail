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

    if sqlx::query("SELECT 1").execute(&mut *conn).await.is_err() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    let api_tokens_table_exists = match sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('core.api_tokens') IS NOT NULL",
    )
    .fetch_one(&mut *conn)
    .await
    {
        Ok(exists) => exists,
        Err(err) => {
            error!("Health check failed to inspect bootstrap schema: {}", err);
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    };

    if !api_tokens_table_exists {
        error!("Health check failed: bootstrap schema is not available yet");
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    let bootstrap_ready = match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM core.api_tokens)"
    )
    .fetch_one(&mut *conn)
    .await
    {
        Ok(ready) => ready,
        Err(err) => {
            error!("Health check failed to confirm bootstrap completion: {}", err);
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    };

    if bootstrap_ready {
        StatusCode::OK
    } else {
        error!("Health check failed: curator bootstrap has not completed yet");
        StatusCode::SERVICE_UNAVAILABLE
    }
}
