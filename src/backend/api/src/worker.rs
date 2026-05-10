use apalis::prelude::*;
use apalis_redis::RedisStorage;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::error;

use crate::error::ApiError;
use common::jobs::SymbolJob;

#[async_trait]
pub trait Worker: Send + Sync + Debug + 'static {
    async fn queue_symbol(&self, symbol_info: serde_json::Value) -> Result<String, ApiError>;
}

#[derive(Clone)]
pub struct WorkQueue {
    symbol_storage: RedisStorage<SymbolJob>,
}

impl std::fmt::Debug for WorkQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkQueue").finish()
    }
}

impl WorkQueue {
    pub fn new(symbol_storage: RedisStorage<SymbolJob>) -> Self {
        WorkQueue { symbol_storage }
    }
}

#[async_trait]
impl Worker for WorkQueue {
    async fn queue_symbol(&self, symbol_info: serde_json::Value) -> Result<String, ApiError> {
        let upload_id = symbol_info
            .get("symbol_upload_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .unwrap_or("unknown".to_string());

        self.symbol_storage
            .clone()
            .push(SymbolJob { symbol_info })
            .await
            .map_err(|e| {
                error!("Failed to queue symbol job: {:?}", e);
                ApiError::Failure("failed to queue symbol job".to_string())
            })?;

        Ok(upload_id)
    }
}

#[derive(Debug, Clone)]
pub struct TestWorker {
    requests: Arc<Mutex<Vec<String>>>,
    failure: Arc<Mutex<bool>>,
}

impl TestWorker {
    pub fn new() -> Self {
        TestWorker {
            requests: Arc::new(Mutex::new(Vec::new())),
            failure: Arc::new(Mutex::new(false)),
        }
    }
    pub fn failure(&self) {
        if let Ok(mut failure) = self.failure.lock() {
            *failure = true;
        }
    }
}

impl Default for TestWorker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Worker for TestWorker {
    async fn queue_symbol(&self, symbol_info: serde_json::Value) -> Result<String, ApiError> {
        if let Ok(failure) = self.failure.lock()
            && *failure
        {
            return Err(ApiError::Failure("failed to queue symbol job".to_string()));
        }
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(symbol_info.to_string());
        }
        Ok(symbol_info["symbol_upload_id"].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_worker_records_requests_and_returns_upload_id_value() {
        let worker = TestWorker::new();
        let upload_id = worker
            .queue_symbol(json!({"symbol_upload_id": "upload-1", "module_id": "app.pdb"}))
            .await
            .unwrap();

        assert_eq!(upload_id, "\"upload-1\"");
        let requests = worker.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("upload-1"));
    }

    #[test]
    fn test_worker_default_creates_empty_worker() {
        let worker = TestWorker::default();
        assert!(worker.requests.lock().unwrap().is_empty());
        assert!(!*worker.failure.lock().unwrap());
    }

    #[tokio::test]
    async fn test_worker_can_be_forced_to_fail() {
        let worker = TestWorker::new();
        worker.failure();

        assert!(matches!(
            worker.queue_symbol(json!({"symbol_upload_id": "upload-1"})).await,
            Err(ApiError::Failure(message)) if message == "failed to queue symbol job"
        ));
    }
}
