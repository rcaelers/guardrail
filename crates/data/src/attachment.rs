use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Attachment {
    pub id: uuid::Uuid,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewAttachment {
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

