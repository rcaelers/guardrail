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

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn new_stores_repo_settings_and_storage() {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        let repo = Repo::new(db);
        let settings = Arc::new(Settings::default());
        let storage: Arc<dyn ObjectStore> = Arc::new(InMemory::new());

        let state = AppState::new(repo, settings.clone(), storage.clone());

        assert!(Arc::ptr_eq(&state.settings, &settings));
        assert!(Arc::ptr_eq(&state.storage, &storage));
    }
}
