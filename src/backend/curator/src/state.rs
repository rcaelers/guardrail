use object_store::ObjectStore;
use std::sync::Arc;

use crate::settings::Settings;
use repos::Repo;

#[derive(Debug, Clone)]
pub struct AppState {
    // Share the authenticated SurrealDB session across workers. Cloning Repo
    // clones its Surreal handle, which creates a new server-side session whose
    // attach/signin/use setup can race the worker's first query.
    pub repo: Arc<Repo>,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
}

impl AppState {
    pub fn new(repo: Arc<Repo>, settings: Arc<Settings>, storage: Arc<dyn ObjectStore>) -> Self {
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
        let repo = Arc::new(Repo::new(db));
        let settings = Arc::new(Settings::default());
        let storage: Arc<dyn ObjectStore> = Arc::new(InMemory::new());

        let state = AppState::new(repo.clone(), settings.clone(), storage.clone());
        let cloned_state = state.clone();

        assert!(Arc::ptr_eq(&state.settings, &settings));
        assert!(Arc::ptr_eq(&state.storage, &storage));
        assert!(Arc::ptr_eq(&state.repo, &repo));
        assert!(Arc::ptr_eq(&state.repo, &cloned_state.repo));
    }
}
