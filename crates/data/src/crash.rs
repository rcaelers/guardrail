use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Crash {
    pub id: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub minidump: Option<uuid::Uuid>,
    pub info: Option<String>,
    pub report: Option<serde_json::Value>,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub build_id: Option<String>,
    pub commit: Option<String>,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewCrash {
    pub id: Option<uuid::Uuid>,
    pub minidump: Option<uuid::Uuid>,
    pub info: Option<String>,
    pub report: Option<serde_json::Value>,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub build_id: Option<String>,
    pub commit: Option<String>,
    pub product_id: uuid::Uuid,
}
