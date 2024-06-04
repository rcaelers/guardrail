pub mod error;
pub mod passkeys;

use cfg_if::cfg_if;
use leptos::{create_rw_signal, RwSignal};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    pub mod layer;
    pub mod extract;

    use crate::entity;
    use tower_sessions::Session;
    use tracing::warn;
}}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
}

impl AuthenticatedUser {
    #[cfg(feature = "ssr")]
    pub fn new(user: entity::user::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
        }
    }
}
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: RwSignal<Option<String>>,
}
impl AuthContext {
    pub fn new(auth_user: Option<AuthenticatedUser>) -> Self {
        let user = create_rw_signal(auth_user.clone().map(|u| u.username));
        AuthContext { user }
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: Option<AuthenticatedUser>,
    pub session: Session,
}

#[cfg(feature = "ssr")]
impl AuthSession {
    fn new(session: Session, user: Option<AuthenticatedUser>) -> Self {
        AuthSession {
            user: user.clone(),
            session,
        }
    }

    pub async fn logout(&mut self) -> Result<(), crate::auth::error::AuthError> {
        warn!("Logging out user: {:?}", self.user);
        let r = self.session.flush().await;
        if let Err(e) = r {
            warn!("Failed to flush session: {:?}", e);
            return Err(crate::auth::error::AuthError::LogoutError(
                "Failed to log out".to_string(),
            ));
        }
        Ok(())
    }
}
