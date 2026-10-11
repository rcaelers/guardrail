use futures::StreamExt;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use surrealdb::Notification;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info, warn};

use common::product_info::{product_cache_keys, product_token_cache_key};
use data::product::Product;

use crate::product_sync::sync_product_by_id;
use crate::settings::Settings;

type ListenerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const RESTART_DELAY: Duration = Duration::from_secs(5);

/// Starts the products, product_settings and validation_scripts listeners.
///
/// Each listener runs on its own SurrealDB connection and opens a new one on
/// every restart, so a broken session (as seen after SurrealDB outages) cannot
/// outlive a retry.
pub fn spawn_product_listeners(settings: Arc<Settings>, redis: ConnectionManager) {
    spawn_listener("products", settings.clone(), redis.clone(), run_products_listener);
    spawn_listener(
        "product_settings",
        settings.clone(),
        redis.clone(),
        run_product_settings_listener,
    );
    spawn_listener("validation_scripts", settings, redis, run_validation_scripts_listener);
}

fn spawn_listener<F, Fut>(
    name: &'static str,
    settings: Arc<Settings>,
    redis: ConnectionManager,
    run: F,
) where
    F: Fn(Surreal<Any>, ConnectionManager) -> Fut + Send + 'static,
    Fut: Future<Output = ListenerResult> + Send,
{
    tokio::spawn(async move {
        loop {
            match crate::db::connect(&settings.database).await {
                Ok(db) => match run(db, redis.clone()).await {
                    Ok(()) => {
                        warn!(listener = name, "LIVE SELECT stream ended unexpectedly, restarting")
                    }
                    Err(e) => error!(listener = name, error = %e, "Listener failed, restarting"),
                },
                Err(e) => {
                    error!(listener = name, error = %e, "Listener cannot connect to SurrealDB")
                }
            }
            tokio::time::sleep(RESTART_DELAY).await;
        }
    });
}

async fn run_products_listener(db: Surreal<Any>, mut redis: ConnectionManager) -> ListenerResult {
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

    Ok(())
}

async fn run_product_settings_listener(
    db: Surreal<Any>,
    mut redis: ConnectionManager,
) -> ListenerResult {
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
                    sync_product_by_id(&db, &pid, &mut redis).await;
                }
            }
            Err(e) => {
                error!(error = ?e, "Error in product_settings notification");
            }
        }
    }

    Ok(())
}

async fn run_validation_scripts_listener(
    db: Surreal<Any>,
    mut redis: ConnectionManager,
) -> ListenerResult {
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
                    sync_product_by_id(&db, &pid, &mut redis).await;
                }
            }
            Err(e) => {
                error!(error = ?e, "Error in validation_scripts notification");
            }
        }
    }

    Ok(())
}
