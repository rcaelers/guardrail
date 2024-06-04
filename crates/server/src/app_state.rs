use axum::extract::FromRef;
use leptos::LeptosOptions;
use leptos_router::RouteListing;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use webauthn_rs::prelude::*;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub routes: Vec<RouteListing>,
    pub db: DatabaseConnection,
    pub webauthn: Arc<Webauthn>,
}
