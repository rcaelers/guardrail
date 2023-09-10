use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::auth::oidc::OidcClientTrait;

pub struct AppState {
    pub db: DatabaseConnection,
    pub auth_client: Arc<dyn OidcClientTrait + Sync + Send>,
}
