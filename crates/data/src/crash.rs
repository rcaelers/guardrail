use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Crash {
    pub id: uuid::Uuid,
    pub state: State,
    pub minidump: Option<uuid::Uuid>,
    pub info: Option<String>,
    pub report: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewCrash {
    pub minidump: uuid::Uuid,
    pub info: Option<String>,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, EnumString, Display, Default, PartialEq)]
#[cfg_attr(feature = "ssr", derive(sqlx::Type))]
#[cfg_attr(feature = "ssr", sqlx(type_name = "text", rename_all = "lowercase"))]
pub enum State {
    #[default]
    #[strum(serialize = "pending")]
    Pending,
    #[strum(serialize = "complete")]
    Complete,
    #[strum(serialize = "failed")]
    Failed,
}
