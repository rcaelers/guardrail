use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Crash {
    pub id: uuid::Uuid,
    pub summary: String,
    pub report: serde_json::Value,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewCrash {
    pub summary: String,
    pub report: serde_json::Value,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

