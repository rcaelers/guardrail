use axum::extract::FromRef;
use common::settings::Settings;
use object_store::ObjectStore;
use repos::Repo;
use std::sync::Arc;
use webauthn_rs::prelude::*;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub repo: Repo,
    pub webauthn: Arc<Webauthn>,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
}
