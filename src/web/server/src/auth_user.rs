use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub is_admin: bool,
    pub avatar: Option<String>,
}

/// Authenticated session state stored in the tower session.
///
/// `user` is the effective (possibly impersonated) user; it is `Some` whenever
/// the session is authenticated.  `real_user` holds the actual admin who
/// initiated impersonation and is `None` during ordinary sessions.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AuthenticatedUser {
    pub user: Option<User>,
    pub real_user: Option<User>,
    /// The OIDC id_token issued at login; used as id_token_hint for RP-initiated logout.
    pub id_token: Option<String>,
}

impl AuthenticatedUser {
    pub fn authenticated(user: User) -> Self {
        Self {
            user: Some(user),
            real_user: None,
            id_token: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    pub fn is_impersonating(&self) -> bool {
        self.real_user.is_some()
    }

    pub fn is_admin(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.is_admin)
    }

    /// Returns the active user.  Only call after a session guard has confirmed
    /// authentication — panics if `user` is `None`.
    pub fn active(&self) -> &User {
        self.user
            .as_ref()
            .expect("active() called on unauthenticated session")
    }

    pub fn current_name(&self) -> &str {
        self.user.as_ref().map_or("", |u| u.name.as_str())
    }
}
