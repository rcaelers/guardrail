use object_store::ObjectStore;
use std::sync::Arc;

use common::settings::Settings;

#[derive(Debug, Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
}

impl AppState {
    pub fn new(settings: Arc<Settings>, storage: Arc<dyn ObjectStore>) -> Self {
        Self { settings, storage }
    }
}
