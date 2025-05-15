use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Symbols {
    pub id: uuid::Uuid,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_location: String,
    pub product_id: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewSymbols {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_location: String,
    pub product_id: uuid::Uuid,
}
