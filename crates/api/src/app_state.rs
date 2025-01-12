use axum::extract::FromRef;
use sea_orm::DatabaseConnection;

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}
