use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub storage_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub crash_id: String,
    pub product_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewAttachment {
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub storage_path: String,
    pub crash_id: String,
    pub product_id: String,
}
