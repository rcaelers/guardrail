use apalis::prelude::*;
use apalis_redis::{ConnectionManager, RedisConfig, RedisStorage};
use async_trait::async_trait;
use common::import_failure::ImportKind;
use common::jobs::{RetryImportJob, queue};

#[async_trait]
pub trait ImportRetryRequestQueue: Send + Sync + 'static {
    async fn enqueue(&self, product_id: &str, kind: ImportKind, id: &str) -> Result<(), String>;
}

pub struct ValkeyImportRetryRequestQueue {
    uri: String,
    connection: tokio::sync::Mutex<Option<ConnectionManager>>,
}

impl ValkeyImportRetryRequestQueue {
    pub fn new(uri: String) -> Self {
        Self {
            uri,
            connection: tokio::sync::Mutex::new(None),
        }
    }

    async fn connection(&self) -> Result<ConnectionManager, String> {
        let mut connection = self.connection.lock().await;
        if let Some(connection) = connection.as_ref() {
            return Ok(connection.clone());
        }
        let connected = apalis_redis::connect(self.uri.clone())
            .await
            .map_err(|error| format!("connect to import queue: {error}"))?;
        *connection = Some(connected.clone());
        Ok(connected)
    }
}

#[async_trait]
impl ImportRetryRequestQueue for ValkeyImportRetryRequestQueue {
    async fn enqueue(&self, product_id: &str, kind: ImportKind, id: &str) -> Result<(), String> {
        let connection = self.connection().await?;
        RedisStorage::new_with_config(connection, RedisConfig::new(queue::RETRY_IMPORT_JOBS))
            .push(RetryImportJob {
                product_id: product_id.to_string(),
                kind,
                import_id: id.to_string(),
            })
            .await
            .map(|_| ())
            .map_err(|error| format!("queue import retry request: {error}"))
    }
}

#[cfg(test)]
pub struct NoopImportRetryRequestQueue;

#[cfg(test)]
#[async_trait]
impl ImportRetryRequestQueue for NoopImportRetryRequestQueue {
    async fn enqueue(&self, _product_id: &str, _kind: ImportKind, _id: &str) -> Result<(), String> {
        Ok(())
    }
}
