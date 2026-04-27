use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use tracing::{error, info};

use common::QueryParams;
use common::product_info::{ProductInfo, product_cache_keys};
use repos::Repo;
use repos::product::ProductRepo;

pub async fn sync_products_to_valkey(
    repo: &Repo,
    redis: &mut ConnectionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let products = ProductRepo::get_all(&repo.db, QueryParams::default()).await?;

    for product in &products {
        let info = ProductInfo {
            id: product.id.clone(),
            name: product.name.clone(),
            accepting_crashes: product.accepting_crashes,
            metadata: product.metadata.clone(),
        };

        let json = serde_json::to_string(&info)?;
        for key in product_cache_keys(&product.name, Some(&product.slug)) {
            info!(product = %product.name, key = %key, "Syncing product to Valkey");

            redis.set::<_, _, ()>(&key, &json).await.map_err(|e| {
                error!(product = %product.name, key = %key, error = ?e, "Failed to write product to Valkey");
                e
            })?;
        }
    }

    info!(count = products.len(), "Synced products to Valkey");
    Ok(())
}
