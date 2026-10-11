use futures::StreamExt;
use redis::aio::ConnectionManager;
use redis::io::tcp::{TcpSettings, socket2::TcpKeepalive};
use redis::{AsyncCommands, IntoConnectionInfo};
use std::time::Duration;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{debug, error, info, warn};

/// Channel the watcher subscribes to; nothing is published on it, the
/// subscription only exists so a dropped connection is noticed at once.
const WATCH_CHANNEL: &str = "guardrail::product:cache-watch";
const RECONNECT_MIN_DELAY: Duration = Duration::from_millis(500);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const RESYNC_MAX_DELAY: Duration = Duration::from_secs(30);

use common::QueryParams;
use common::product_info::{
    CachedProcessorSettings, CachedValidationScript, PRODUCT_CACHE_READY_KEY, ProductInfo,
    product_cache_keys, product_token_cache_key,
};
use data::product::Product;
use repos::Repo;
use repos::product::ProductRepo;
use repos::product_settings::ProductSettingsRepo;
use repos::validation_scripts::ValidationScriptsRepo;

async fn build_product_info(db: &Surreal<Any>, product: &Product) -> ProductInfo {
    let settings = match ProductSettingsRepo::get(db, &product.id).await {
        Ok(s) => s.unwrap_or_default(),
        Err(e) => {
            error!(product = %product.name, error = ?e, "Failed to fetch product settings for cache sync");
            Default::default()
        }
    };

    let scripts = match ValidationScriptsRepo::list(db, &product.id).await {
        Ok(s) => s,
        Err(e) => {
            error!(product = %product.name, error = ?e, "Failed to fetch validation scripts for cache sync");
            vec![]
        }
    };

    let processor_settings = Some(CachedProcessorSettings {
        skip_patterns: settings.processor.skip_patterns,
        end_patterns: settings.processor.end_patterns,
        delimiter: settings.processor.delimiter,
        maximum_frame_count: settings.processor.maximum_frame_count.map(|n| n as usize),
        skip_untrusted_frames: settings.processor.skip_untrusted_frames,
        fold_module_case: settings.processor.fold_module_case,
    });

    let cached_scripts: Vec<CachedValidationScript> = scripts
        .into_iter()
        .map(|s| CachedValidationScript {
            id: s.id,
            name: s.name,
            content: s.content,
        })
        .collect();

    ProductInfo {
        id: product.id.clone(),
        name: product.name.clone(),
        accepting_crashes: product.accepting_crashes,
        metadata: product.metadata.clone(),
        mandatory_annotations: settings.minidump.mandatory_annotations.unwrap_or_default(),
        validation_scripts: cached_scripts,
        processor_settings,
    }
}

/// Returns whether every key was written.
async fn write_product_to_redis(
    product: &Product,
    info: &ProductInfo,
    redis: &mut ConnectionManager,
) -> bool {
    let json = match serde_json::to_string(info) {
        Ok(j) => j,
        Err(e) => {
            error!(product = %product.name, error = ?e, "Failed to serialize product info");
            return false;
        }
    };

    let mut keys = product_cache_keys(&product.name, Some(&product.slug));
    keys.push(product_token_cache_key(&product.product_token));

    let mut written = true;
    for key in keys {
        info!(product = %product.name, key = %key, "Syncing product to Valkey");
        if let Err(e) = redis.set::<_, _, ()>(&key, &json).await {
            error!(product = %product.name, key = %key, error = ?e, "Failed to write product to Valkey");
            written = false;
        }
    }
    written
}

pub async fn sync_products_to_valkey(
    repo: &Repo,
    redis: &mut ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let products = ProductRepo::get_all(&repo.db, QueryParams::default()).await?;

    let mut complete = true;
    for product in &products {
        let info = build_product_info(&repo.db, product).await;
        complete &= write_product_to_redis(product, &info, redis).await;
    }
    if !complete {
        return Err("failed to write some products to Valkey".into());
    }

    // Written last, so ingestion only trusts a cache miss once every product is in.
    redis.set::<_, _, ()>(PRODUCT_CACHE_READY_KEY, "1").await?;

    info!(count = products.len(), "Synced products to Valkey");
    Ok(())
}

/// Keeps the product cache filled across Valkey restarts without polling.
///
/// Holds an idle pub/sub connection to Valkey, which costs nothing while Valkey
/// is up. When Valkey restarts or is replaced the connection closes; the watcher
/// then reconnects with backoff and, once Valkey is back, resyncs every product
/// if the cache-ready marker is gone. Until then ingestion answers 503, so
/// clients retry instead of losing crashes. TCP keepalive (handled by the
/// kernel) detects a Valkey that vanished without closing the connection.
pub async fn watch_product_cache(repo: &Repo, valkey_uri: &str, mut redis: ConnectionManager) {
    let client = match watcher_client(valkey_uri) {
        Ok(client) => client,
        Err(e) => {
            error!("Product cache watcher disabled, invalid Valkey URI: {}", e);
            return;
        }
    };

    let mut delay = RECONNECT_MIN_DELAY;
    loop {
        match client.get_async_pubsub().await {
            Ok(mut pubsub) => {
                if let Err(e) = pubsub.subscribe(WATCH_CHANNEL).await {
                    debug!("Failed to subscribe to Valkey: {}", e);
                } else {
                    delay = RECONNECT_MIN_DELAY;
                    // Valkey may be new or empty after we were disconnected.
                    ensure_product_cache(repo, &mut redis).await;
                    let mut messages = pubsub.into_on_message();
                    while messages.next().await.is_some() {}
                    warn!("Lost connection to Valkey, waiting for it to return");
                }
            }
            Err(e) => debug!("Valkey unavailable: {}", e),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(RECONNECT_MAX_DELAY);
    }
}

fn watcher_client(valkey_uri: &str) -> redis::RedisResult<redis::Client> {
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(30))
        .with_interval(Duration::from_secs(10));
    let info = valkey_uri
        .into_connection_info()?
        .set_tcp_settings(TcpSettings::default().set_keepalive(keepalive));
    redis::Client::open(info)
}

/// Resyncs when the cache-ready marker is missing, retrying while the database
/// is unavailable.
async fn ensure_product_cache(repo: &Repo, redis: &mut ConnectionManager) {
    let mut delay = RECONNECT_MIN_DELAY;
    loop {
        match redis.exists::<_, bool>(PRODUCT_CACHE_READY_KEY).await {
            Ok(true) => return,
            Ok(false) => {
                warn!("Product cache in Valkey is empty, resyncing products");
                match sync_products_to_valkey(repo, redis).await {
                    Ok(()) => return,
                    Err(e) => error!("Failed to resync products to Valkey: {}", e),
                }
            }
            Err(e) => debug!("Failed to check product cache marker: {}", e),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(RESYNC_MAX_DELAY);
    }
}

pub async fn sync_product_by_id(
    db: &Surreal<Any>,
    product_id: &str,
    redis: &mut ConnectionManager,
) {
    let product = match ProductRepo::get_by_id(db, product_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            error!(product_id, "Product not found during sync");
            return;
        }
        Err(e) => {
            error!(product_id, error = ?e, "Failed to fetch product during sync");
            return;
        }
    };

    let info = build_product_info(db, &product).await;
    write_product_to_redis(&product, &info, redis).await;
}
