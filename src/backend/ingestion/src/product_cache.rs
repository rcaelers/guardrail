use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use tracing::error;

use common::product_info::{ProductInfo, product_cache_key};

use crate::error::ApiError;

#[derive(Debug, Clone)]
enum Backend {
    Redis(ConnectionManager),
    Memory(HashMap<String, ProductInfo>),
}

#[derive(Debug, Clone)]
pub struct ProductCache {
    backend: Backend,
}

impl ProductCache {
    pub fn new(redis: ConnectionManager) -> Self {
        Self {
            backend: Backend::Redis(redis),
        }
    }

    pub fn from_map(products: HashMap<String, ProductInfo>) -> Self {
        Self {
            backend: Backend::Memory(products),
        }
    }

    pub async fn get_product_by_name(&self, name: &str) -> Result<Option<ProductInfo>, ApiError> {
        match &self.backend {
            Backend::Redis(conn) => {
                let key = product_cache_key(name);
                let json: Option<String> = conn
                    .clone()
                    .get(&key)
                    .await
                    .map_err(|e| {
                        error!(error = ?e, "Failed to get product from Valkey");
                        ApiError::Failure("failed to get product info".to_string())
                    })?;

                match json {
                    Some(j) => {
                        let info: ProductInfo = serde_json::from_str(&j).map_err(|e| {
                            error!(error = ?e, "Failed to deserialize product info");
                            ApiError::Failure("failed to deserialize product info".to_string())
                        })?;
                        Ok(Some(info))
                    }
                    None => Ok(None),
                }
            }
            Backend::Memory(map) => Ok(map.get(name).cloned()),
        }
    }

    pub async fn is_healthy(&self) -> bool {
        match &self.backend {
            Backend::Redis(conn) => {
                let result: Result<String, _> =
                    redis::cmd("PING").query_async(&mut conn.clone()).await;
                result.is_ok()
            }
            Backend::Memory(_) => true,
        }
    }
}
