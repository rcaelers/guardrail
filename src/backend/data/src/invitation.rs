use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InvitationStatus {
    Active,
    Exhausted,
    Expired,
    Revoked,
}

/// One product/role pair carried on an invitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationGrant {
    pub product_id: String,
    /// One of "readonly" | "readwrite" | "maintainer"
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: String,
    pub code: String,
    pub created_by: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<u32>,
    pub use_count: u32,
    pub is_admin: bool,
    pub grants: Vec<InvitationGrant>,
    pub status: InvitationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewInvitation {
    pub created_by: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<u32>,
    pub is_admin: bool,
    pub grants: Vec<InvitationGrant>,
}
