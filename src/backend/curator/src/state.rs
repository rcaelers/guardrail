use object_store::ObjectStore;
use std::sync::Arc;

use common::settings::Settings;
use repos::Repo;

#[derive(Debug, Clone)]
pub struct AppState {
    pub repo: Repo,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
}

impl AppState {
    pub fn new(repo: Repo, settings: Arc<Settings>, storage: Arc<dyn ObjectStore>) -> Self {
        Self {
            repo,
            settings,
            storage,
        }
    }
}
