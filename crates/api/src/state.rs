use axum::extract::FromRef;
use repos::Repo;
use std::sync::Arc;
use webauthn_rs::prelude::*;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub repo: Repo,
    pub webauthn: Arc<Webauthn>,
}
