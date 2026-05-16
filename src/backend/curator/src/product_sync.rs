use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info};

use common::QueryParams;
use common::product_info::{
    CachedProcessorSettings, CachedValidationScript, ProductInfo, product_cache_keys,
    product_token_cache_key,
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

async fn write_product_to_redis(
    product: &Product,
    info: &ProductInfo,
    redis: &mut ConnectionManager,
) {
    let json = match serde_json::to_string(info) {
        Ok(j) => j,
        Err(e) => {
            error!(product = %product.name, error = ?e, "Failed to serialize product info");
            return;
        }
    };

    let mut keys = product_cache_keys(&product.name, Some(&product.slug));
    keys.push(product_token_cache_key(&product.product_token));

    for key in keys {
        info!(product = %product.name, key = %key, "Syncing product to Valkey");
        if let Err(e) = redis.set::<_, _, ()>(&key, &json).await {
            error!(product = %product.name, key = %key, error = ?e, "Failed to write product to Valkey");
        }
    }
}

pub async fn sync_products_to_valkey(
    repo: &Repo,
    redis: &mut ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let products = ProductRepo::get_all(&repo.db, QueryParams::default()).await?;

    for product in &products {
        let info = build_product_info(&repo.db, product).await;
        write_product_to_redis(product, &info, redis).await;
    }

    info!(count = products.len(), "Synced products to Valkey");
    Ok(())
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
