use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub name: String,
    pub avatar: String,
    pub is_admin: bool,
    /// Stable identity-provider subject (OIDC `sub`). `None` for the anonymous
    /// record and legacy accounts that predate sub-binding.
    #[serde(default)]
    pub sub: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewUser {
    pub username: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub is_admin: bool,
    /// OIDC `sub` to bind this account to. Should be set for every
    /// provider-provisioned user.
    pub sub: Option<String>,
}
