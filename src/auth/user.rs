use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
}

impl AuthenticatedUser {
    pub fn new(user: entity::user::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
        }
    }
}
