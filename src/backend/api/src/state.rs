use axum::extract::FromRef;
use object_store::ObjectStore;
use std::sync::Arc;

use crate::settings::Settings;
use crate::worker::Worker;
use repos::Repo;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    // Shared, not cloned per request: cloning a Surreal handle opens a new
    // server-side session whose signin and use are replayed without waiting,
    // so a request's first query could run before they land.
    pub repo: Arc<Repo>,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
    pub worker: Arc<dyn Worker>,
}
