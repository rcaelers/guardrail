use axum::extract::FromRef;
use object_store::ObjectStore;
use std::sync::Arc;

use crate::product_cache::ProductCache;
use crate::worker::Worker;
use common::settings::Settings;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub product_cache: ProductCache,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
    pub worker: Arc<dyn Worker>,
}
