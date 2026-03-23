use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub id: Uuid,
    pub name: String,
    pub accepting_crashes: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn product_cache_key(product_name: &str) -> String {
    format!("product:by-name:{product_name}")
}
