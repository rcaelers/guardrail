use common::AuthenticatedUser;

#[derive(Clone, Debug, Default)]
pub struct AuthSession {
    pub user: Option<AuthenticatedUser>,
}

impl AuthSession {
    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    pub fn current_username(&self) -> &str {
        self.user
            .as_ref()
            .map(|user| user.username.as_str())
            .unwrap_or("")
    }
}
