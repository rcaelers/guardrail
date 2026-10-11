use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use tracing::{error, info, warn};

use common::product_info::{
    PRODUCT_CACHE_READY_KEY, ProductInfo, product_cache_key, product_token_cache_key,
};

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

    pub fn from_token_map(products: HashMap<String, ProductInfo>) -> Self {
        let products = products
            .into_iter()
            .map(|(token, product)| (product_token_cache_key(&token), product))
            .collect();

        Self {
            backend: Backend::Memory(products),
        }
    }

    pub async fn get_product_by_token(&self, token: &str) -> Result<Option<ProductInfo>, ApiError> {
        match &self.backend {
            Backend::Redis(conn) => {
                let key = product_token_cache_key(token);
                info!(token = %token, key = %key, "Fetching product from Valkey by token");
                let json: Option<String> = conn.clone().get(&key).await.map_err(|e| {
                    error!(token = %token, key = %key, error = ?e, "Failed to get product from Valkey");
                    ApiError::ServiceUnavailable("cache unavailable".to_string())
                })?;

                match json {
                    Some(j) => {
                        let info: ProductInfo = serde_json::from_str(&j).map_err(|e| {
                            error!(token = %token, key = %key, error = ?e, "Failed to deserialize product info");
                            ApiError::Failure("failed to deserialize product info".to_string())
                        })?;
                        Ok(Some(info))
                    }
                    None => {
                        Self::ensure_cache_ready(conn).await?;
                        Ok(None)
                    }
                }
            }
            Backend::Memory(map) => Ok(map.get(&product_token_cache_key(token)).cloned()),
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

    /// Ready only once the curator has filled the cache; with an empty cache
    /// every upload would be rejected as an unknown product.
    pub async fn is_healthy(&self) -> bool {
        match &self.backend {
            Backend::Redis(conn) => conn
                .clone()
                .exists::<_, bool>(PRODUCT_CACHE_READY_KEY)
                .await
                .unwrap_or(false),
            Backend::Memory(_) => true,
        }
    }

    /// A miss only means "unknown product" once the curator has filled the
    /// cache. Before that (new or wiped Valkey) answer 503, so the client keeps
    /// the crash report and retries instead of it being rejected for good.
    async fn ensure_cache_ready(conn: &ConnectionManager) -> Result<(), ApiError> {
        let ready = conn
            .clone()
            .exists::<_, bool>(PRODUCT_CACHE_READY_KEY)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to check product cache marker in Valkey");
                ApiError::ServiceUnavailable("cache unavailable".to_string())
            })?;
        if !ready {
            warn!("Product cache in Valkey is not populated yet");
            return Err(ApiError::ServiceUnavailable("product cache not ready".to_string()));
        }
        Ok(())
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
            mandatory_annotations: vec![],
            validation_scripts: vec![],
            processor_settings: None,
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

    /// Needs a scratch Valkey, e.g. GUARDRAIL_TEST_VALKEY_URI=redis://127.0.0.1:6379/15
    #[tokio::test]
    #[ignore = "requires GUARDRAIL_TEST_VALKEY_URI"]
    async fn valkey_cache_miss_is_unavailable_until_cache_is_ready() {
        let uri = std::env::var("GUARDRAIL_TEST_VALKEY_URI").expect("GUARDRAIL_TEST_VALKEY_URI");
        let client = redis::Client::open(uri).unwrap();
        let mut conn = ConnectionManager::new(client).await.unwrap();
        let _: () = redis::cmd("FLUSHDB").query_async(&mut conn).await.unwrap();
        let cache = ProductCache::new(conn.clone());

        assert!(!cache.is_healthy().await);
        assert!(matches!(
            cache.get_product_by_token("unknown").await,
            Err(ApiError::ServiceUnavailable(_))
        ));

        let _: () = conn.set(PRODUCT_CACHE_READY_KEY, "1").await.unwrap();

        assert!(cache.is_healthy().await);
        assert!(cache.get_product_by_token("unknown").await.unwrap().is_none());
    }
}
