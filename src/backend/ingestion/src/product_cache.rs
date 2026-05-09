use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use tracing::{error, info};

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
        let products = products
            .into_iter()
            .map(|(name, product)| (product_cache_key(&name), product))
            .collect();

        Self {
            backend: Backend::Memory(products),
        }
    }

    pub async fn get_product_by_name(&self, name: &str) -> Result<Option<ProductInfo>, ApiError> {
        match &self.backend {
            Backend::Redis(conn) => {
                let key = product_cache_key(name);
                info!(name = %name, key = %key, "Fetching product from Valkey");
                let json: Option<String> = conn.clone().get(&key).await.map_err(|e| {
                    error!(name = %name, key = %key, error = ?e, "Failed to get product from Valkey");
                    ApiError::ServiceUnavailable("cache unavailable".to_string())
                })?;

                match json {
                    Some(j) => {
                        let info: ProductInfo = serde_json::from_str(&j).map_err(|e| {
                            error!(name = %name, key = %key, error = ?e, "Failed to deserialize product info");
                            ApiError::Failure("failed to deserialize product info".to_string())
                        })?;
                        Ok(Some(info))
                    }
                    None => Ok(None),
                }
            }
            Backend::Memory(map) => Ok(map.get(&product_cache_key(name)).cloned()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use common::product_info::ProductInfo;
    use serde_json::json;

    fn product(name: &str, accepting_crashes: bool) -> ProductInfo {
        ProductInfo {
            id: format!("{name}-id"),
            name: name.to_string(),
            accepting_crashes,
            metadata: json!({"kind": name}),
        }
    }

    #[tokio::test]
    async fn memory_cache_returns_products_by_case_insensitive_key() {
        let cache = ProductCache::from_map(HashMap::from([(
            "Workrave".to_string(),
            product("Workrave", true),
        )]));

        let found = cache
            .get_product_by_name("workrave")
            .await
            .unwrap()
            .expect("product should be found");
        assert_eq!(found.id, "Workrave-id");
        assert!(found.accepting_crashes);
        assert_eq!(found.metadata, json!({"kind": "Workrave"}));
    }

    #[tokio::test]
    async fn memory_cache_returns_none_for_unknown_product() {
        let cache =
            ProductCache::from_map(HashMap::from([("Known".to_string(), product("Known", true))]));

        assert!(
            cache
                .get_product_by_name("unknown")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn memory_cache_is_healthy() {
        let cache = ProductCache::from_map(HashMap::new());
        assert!(cache.is_healthy().await);
    }
}
