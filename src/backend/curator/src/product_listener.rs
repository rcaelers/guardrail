use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tracing::{error, info, warn};

use common::product_info::{ProductInfo, product_cache_key};

#[derive(Debug, serde::Deserialize)]
struct ProductChangePayload {
    op: String,
    id: uuid::Uuid,
    name: String,
    accepting_crashes: bool,
    #[serde(default)]
    metadata: serde_json::Value,
    old_name: Option<String>,
}

pub async fn listen_for_product_changes(
    pool: PgPool,
    mut redis: ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut listener = PgListener::connect_with(&pool).await?;
    listener.listen("product_changed").await?;
    info!("Listening for product_changed notifications");

    loop {
        let notification = listener.recv().await?;
        let payload = notification.payload();

        let change: ProductChangePayload = match serde_json::from_str(payload) {
            Ok(c) => c,
            Err(e) => {
                warn!(payload, error = ?e, "Failed to parse product change payload");
                continue;
            }
        };

        info!(op = %change.op, product = %change.name, id = %change.id, "Product changed");

        match change.op.as_str() {
            "INSERT" | "UPDATE" => {
                let info = ProductInfo {
                    id: change.id,
                    name: change.name.clone(),
                    accepting_crashes: change.accepting_crashes,
                    metadata: change.metadata.clone(),
                };
                let json = serde_json::to_string(&info)?;
                let key = product_cache_key(&change.name);

                if let Err(e) = redis.set::<_, _, ()>(&key, &json).await {
                    error!(product = %change.name, error = ?e, "Failed to write product to Valkey");
                }

                // If the product was renamed, remove the old key
                if let Some(old_name) = &change.old_name {
                    let old_key = product_cache_key(old_name);
                    if let Err(e) = redis.del::<_, ()>(&old_key).await {
                        error!(product = %old_name, error = ?e, "Failed to remove old product key from Valkey");
                    }
                    info!(old_name, new_name = %change.name, "Product renamed, removed old cache key");
                }
            }
            "DELETE" => {
                let key = product_cache_key(&change.name);
                if let Err(e) = redis.del::<_, ()>(&key).await {
                    error!(product = %change.name, error = ?e, "Failed to remove product from Valkey");
                }
                info!(product = %change.name, "Removed deleted product from Valkey");
            }
            _ => {
                warn!(op = %change.op, "Unknown operation in product change notification");
            }
        }
    }
}
