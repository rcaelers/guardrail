use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAccessGrant {
    pub product_id: String,
    pub role: String,
}

/// Temporary record stored between invitation redemption and first OIDC login.
/// Keyed by the identity-provider subject (`sub`).
/// `invitation_id` is used to increment the invitation use-count on first login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAccess {
    pub id: String,
    pub sub: String,
    pub invitation_id: String,
    pub is_admin: bool,
    pub grants: Vec<PendingAccessGrant>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPendingAccess {
    pub sub: String,
    pub invitation_id: String,
    pub is_admin: bool,
    pub grants: Vec<PendingAccessGrant>,
}
