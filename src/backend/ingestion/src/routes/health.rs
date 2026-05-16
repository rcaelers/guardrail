use axum::{extract::State, http::StatusCode};
use tracing::error;

use crate::state::AppState;

pub async fn live() -> StatusCode {
    StatusCode::OK
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
