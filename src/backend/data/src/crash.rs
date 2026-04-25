use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Crash {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub minidump: Option<uuid::Uuid>,
    pub report: Option<serde_json::Value>,
    pub fingerprint: Option<String>,
    pub group_id: Option<String>,
    pub product_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewCrash {
    pub id: Option<String>,
    pub minidump: Option<uuid::Uuid>,
    pub report: Option<serde_json::Value>,
    pub fingerprint: Option<String>,
    pub group_id: Option<String>,
    pub product_id: String,
}
