use futures::StreamExt;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use surrealdb::Notification;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info, warn};

use common::product_info::{product_cache_keys, product_token_cache_key};
use data::product::Product;

use crate::product_sync::sync_product_by_id;

pub async fn listen_for_product_changes(
    db: Surreal<Any>,
    mut redis: ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_settings = db.clone();
    let db_scripts = db.clone();
    let redis_settings = redis.clone();
    let redis_scripts = redis.clone();

    tokio::spawn(listen_for_product_settings_changes(db_settings, redis_settings));
    tokio::spawn(listen_for_validation_script_changes(db_scripts, redis_scripts));

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
                        sync_product_by_id(&db, &product.id, &mut redis).await;
                    }
                    surrealdb::types::Action::Delete => {
                        let mut keys = product_cache_keys(&product.name, Some(&product.slug));
                        keys.push(product_token_cache_key(&product.product_token));
                        for key in keys {
                            if let Err(e) = redis.del::<_, ()>(&key).await {
                                error!(product = %product.name, key = %key, error = ?e, "Failed to remove product from Valkey");
                            }
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

async fn listen_for_product_settings_changes(db: Surreal<Any>, mut redis: ConnectionManager) {
    loop {
        match run_product_settings_listener(&db, &mut redis).await {
            Ok(()) => warn!("product_settings LIVE SELECT stream ended unexpectedly, restarting"),
            Err(e) => error!(error = ?e, "product_settings listener failed, restarting"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn run_product_settings_listener(
    db: &Surreal<Any>,
    redis: &mut ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut result = db
        .query("LIVE SELECT meta::id(product_id) AS product_id FROM product_settings")
        .await?;
    let mut stream: surrealdb::method::QueryStream<Notification<serde_json::Value>> =
        result.stream(0)?;
    info!("Listening for product_settings changes via LIVE SELECT");

    while let Some(notification) = stream.next().await {
        match notification {
            Ok(n) => {
                let product_id = n
                    .data
                    .get("product_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                if let Some(pid) = product_id {
                    info!(product_id = %pid, "product_settings changed, re-syncing");
                    sync_product_by_id(db, &pid, redis).await;
                }
            }
            Err(e) => {
                error!(error = ?e, "Error in product_settings notification");
            }
        }
    }

    Ok(())
}

async fn listen_for_validation_script_changes(db: Surreal<Any>, mut redis: ConnectionManager) {
    loop {
        match run_validation_scripts_listener(&db, &mut redis).await {
            Ok(()) => warn!("validation_scripts LIVE SELECT stream ended unexpectedly, restarting"),
            Err(e) => error!(error = ?e, "validation_scripts listener failed, restarting"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn run_validation_scripts_listener(
    db: &Surreal<Any>,
    redis: &mut ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut result = db
        .query("LIVE SELECT meta::id(product_id) AS product_id FROM validation_scripts")
        .await?;
    let mut stream: surrealdb::method::QueryStream<Notification<serde_json::Value>> =
        result.stream(0)?;
    info!("Listening for validation_scripts changes via LIVE SELECT");

    while let Some(notification) = stream.next().await {
        match notification {
            Ok(n) => {
                let product_id = n
                    .data
                    .get("product_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                if let Some(pid) = product_id {
                    info!(product_id = %pid, "validation_scripts changed, re-syncing");
                    sync_product_by_id(db, &pid, redis).await;
                }
            }
            Err(e) => {
                error!(error = ?e, "Error in validation_scripts notification");
            }
        }
    }

    Ok(())
}
