use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Symbols {
    pub id: String,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_path: String,
    pub product_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewSymbols {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_path: String,
    pub product_id: String,
}
