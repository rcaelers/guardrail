use apalis::prelude::*;
use apalis_sql::postgres::PostgresStorage;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::error;

use crate::error::ApiError;
use jobs::jobs::MinidumpJob;

#[async_trait]
pub trait Worker: Send + Sync + Debug + 'static {
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError>;
}

#[derive(Debug, Clone)]
pub struct MinidumpProcessor {
    worker: PostgresStorage<MinidumpJob>,
}

impl MinidumpProcessor {
    pub fn new(worker: PostgresStorage<MinidumpJob>) -> Self {
        MinidumpProcessor { worker }
    }
}

#[async_trait]
impl Worker for MinidumpProcessor {
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError> {
        let job = self
            .worker
            .clone()
            .push(MinidumpJob { crash })
            .await
            .map_err(|e| {
                error!("Failed to queue minidump job: {:?}", e);
                ApiError::Failure("failed to queue minidump job".to_string())
            })?;
        Ok(job.task_id.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct TestMinidumpProcessor {
    requests: Arc<Mutex<Vec<String>>>,
    failure: Arc<Mutex<bool>>,
}

impl TestMinidumpProcessor {
    pub fn new() -> Self {
        TestMinidumpProcessor {
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

impl Default for TestMinidumpProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Worker for TestMinidumpProcessor {
    async fn queue_minidump(&self, crash: serde_json::Value) -> Result<String, ApiError> {
        if let Ok(failure) = self.failure.lock() {
            if *failure {
                return Err(ApiError::Failure("failed to queue minidump job".to_string()));
            }
        }
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(crash.to_string());
        }
        Ok(crash["id"].to_string())
    }
}
