use sea_orm::DatabaseConnection;
use std::sync::Arc;
use webauthn_rs::prelude::*;

pub struct AppState {
    pub db: DatabaseConnection,
    pub webauthn: Arc<Webauthn>,
}
