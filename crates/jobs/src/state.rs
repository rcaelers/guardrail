use common::settings::Settings;
use object_store::ObjectStore;
use repos::Repo;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppState {
    pub repo: Repo,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
}
