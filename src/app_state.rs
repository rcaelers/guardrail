use sea_orm::DatabaseConnection;

use crate::auth::oidc::OidcClientTraitArc;

pub struct AppState {
    pub db: DatabaseConnection,
    pub auth_client: OidcClientTraitArc,
}
