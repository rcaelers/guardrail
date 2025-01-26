use axum::extract::FromRef;
use leptos::config::LeptosOptions;
use leptos_axum::AxumRouteListing;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use webauthn_rs::prelude::*;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub routes: Vec<AxumRouteListing>,
    pub db: DatabaseConnection,
    pub webauthn: Arc<Webauthn>,
}
