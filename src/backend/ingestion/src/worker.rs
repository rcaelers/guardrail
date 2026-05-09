use apalis::prelude::*;
use apalis_redis::RedisStorage;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::error;

use crate::error::ApiError;
use common::jobs::MinidumpJob;

#[async_trait]
pub trait Worker: Send + Sync + Debug + 'static {
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError>;
}

#[derive(Clone)]
pub struct WorkQueue {
    minidump_storage: RedisStorage<MinidumpJob>,
}

impl std::fmt::Debug for WorkQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkQueue").finish()
    }
}

impl WorkQueue {
    pub fn new(minidump_storage: RedisStorage<MinidumpJob>) -> Self {
        WorkQueue { minidump_storage }
    }
}

#[async_trait]
impl Worker for WorkQueue {
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError> {
        let task_id = crash
            .get("crash_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .unwrap_or("unknown".to_string());

        self.minidump_storage
            .clone()
            .push(MinidumpJob { crash })
            .await
            .map_err(|e| {
                error!("Failed to queue minidump job: {:?}", e);
                ApiError::ServiceUnavailable("job queue unavailable".to_string())
            })?;

        Ok(task_id)
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
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError> {
        if let Ok(failure) = self.failure.lock()
            && *failure
        {
            return Err(ApiError::Failure("failed to queue minidump job".to_string()));
        }
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(crash.to_string());
        }
        Ok(crash["id"].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_worker_records_requests_and_returns_id_value() {
        let worker = TestWorker::new();
        let result = worker
            .queue_minidump(json!({"id": "crash-1"}))
            .await
            .unwrap();

        assert_eq!(result, "\"crash-1\"");
        let requests = worker.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0], json!({"id": "crash-1"}).to_string());
    }

    #[tokio::test]
    async fn test_worker_can_be_forced_to_fail() {
        let worker = TestWorker::default();
        worker.failure();

        let err = worker
            .queue_minidump(json!({"id": "crash-1"}))
            .await
            .unwrap_err();

        assert!(
            matches!(err, ApiError::Failure(message) if message == "failed to queue minidump job")
        );
    }
}
