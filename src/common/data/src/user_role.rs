use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::invitation::Role;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRole {
    pub id: String,
    pub sub: String,
    pub roles: Vec<Role>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserRole {
    pub sub: String,
    pub roles: Vec<Role>,
}
