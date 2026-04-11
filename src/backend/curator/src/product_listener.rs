use futures::StreamExt;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::Notification;
use tracing::{error, info, warn};

use common::product_info::{ProductInfo, product_cache_key};
use data::product::Product;

pub async fn listen_for_product_changes(
    db: Surreal<Any>,
    mut redis: ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut result = db
        .query("LIVE SELECT *, meta::id(id) as id FROM products")
        .await?;
    let mut stream: surrealdb::method::QueryStream<Notification<serde_json::Value>> =
        result.stream(0)?;
    info!("Listening for product changes via LIVE SELECT");

    while let Some(notification) = stream.next().await {
        match notification {
            Ok(notification) => {
                let action = &notification.action;
                let product: Product = match serde_json::from_value(notification.data) {
                    Ok(p) => p,
                    Err(e) => {
                        error!(error = ?e, "Failed to deserialize product notification");
                        continue;
                    }
                };

                info!(action = ?action, product = %product.name, id = %product.id, "Product changed");

                match action {
                    surrealdb::types::Action::Create | surrealdb::types::Action::Update => {
                        let info = ProductInfo {
                            id: product.id,
                            name: product.name.clone(),
                            accepting_crashes: product.accepting_crashes,
                            metadata: product.metadata.clone(),
                        };
                        let json = serde_json::to_string(&info)?;
                        let key = product_cache_key(&product.name);

                        if let Err(e) = redis.set::<_, _, ()>(&key, &json).await {
                            error!(product = %product.name, error = ?e, "Failed to write product to Valkey");
                        }
                    }
                    surrealdb::types::Action::Delete => {
                        let key = product_cache_key(&product.name);
                        if let Err(e) = redis.del::<_, ()>(&key).await {
                            error!(product = %product.name, error = ?e, "Failed to remove product from Valkey");
                        }
                        info!(product = %product.name, "Removed deleted product from Valkey");
                    }
                    _ => {
                        warn!(action = ?action, "Unknown action in product change notification");
                    }
                }
            }
            Err(e) => {
                error!(error = ?e, "Error receiving product change notification");
            }
        }
    }

    warn!("LIVE SELECT stream ended unexpectedly");
    Ok(())
}
