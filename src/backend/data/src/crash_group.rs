use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CrashGroup {
    pub id: String,
    pub product_id: String,
    pub fingerprint: String,
    pub signal: String,
    pub count: i64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewCrashGroup {
    pub product_id: String,
    pub fingerprint: String,
    /// Human-readable display label — typically mirrors the fingerprint.
    pub signal: String,
}
