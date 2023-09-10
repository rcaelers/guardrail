use sea_orm::DatabaseConnection;

use crate::auth::oidc::OidcClient;

#[derive(Debug)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub auth_client: OidcClient,
}
