use axum::extract::FromRef;
use object_store::ObjectStore;
use std::sync::Arc;
use webauthn_rs::prelude::*;

use crate::worker::Worker;
use common::settings::Settings;
use repos::Repo;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub repo: Repo,
    pub webauthn: Arc<Webauthn>,
    pub settings: Arc<Settings>,
    pub storage: Arc<dyn ObjectStore>,
    pub worker: Arc<dyn Worker>,
}
