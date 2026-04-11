use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Crash {
    pub id: uuid::Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub minidump: Option<uuid::Uuid>,
    pub report: Option<serde_json::Value>,
    pub signature: Option<String>,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewCrash {
    pub id: Option<uuid::Uuid>,
    pub minidump: Option<uuid::Uuid>,
    pub report: Option<serde_json::Value>,
    pub signature: Option<String>,
    pub product_id: uuid::Uuid,
}
