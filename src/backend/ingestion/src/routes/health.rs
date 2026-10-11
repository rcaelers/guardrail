use axum::{Json, extract::State, http::StatusCode};
use tracing::error;

use crate::upload_stats::UploadHealth;

use crate::state::AppState;

pub async fn live() -> StatusCode {
    StatusCode::OK
}

/// Fails when real uploads are being rejected while the service itself is up.
/// Internal only: the public route only forwards /api/minidump.
pub async fn upload_health(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.upload_stats.health().await {
        Ok(health) => {
            let status = if health.healthy {
                StatusCode::OK
            } else {
                error!(problems = ?health.problems, "Uploads are being rejected");
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(health_json(&health)))
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "healthy": false, "problems": [e.to_string()] })),
        ),
    }
}

fn health_json(health: &UploadHealth) -> serde_json::Value {
    serde_json::to_value(health).unwrap_or_default()
}

pub async fn ready(State(state): State<AppState>) -> StatusCode {
    if state.product_cache.is_healthy().await {
        StatusCode::OK
    } else {
        error!("Health check failed: Valkey is not reachable");
        StatusCode::SERVICE_UNAVAILABLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product_cache::ProductCache;
    use crate::worker::TestWorker;
    use object_store::memory::InMemory;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn state() -> AppState {
        AppState {
            product_cache: ProductCache::from_map(HashMap::new()),
            settings: Arc::new(crate::settings::Settings::test_default()),
            storage: Arc::new(InMemory::new()),
            worker: Arc::new(TestWorker::new()),
            rate_limiter: None,
            upload_stats: crate::upload_stats::UploadStats::memory(),
        }
    }

    #[tokio::test]
    async fn live_returns_ok() {
        assert_eq!(live().await, StatusCode::OK);
    }

    #[tokio::test]
    async fn ready_returns_ok_when_cache_is_healthy() {
        assert_eq!(ready(State(state())).await, StatusCode::OK);
    }
}
