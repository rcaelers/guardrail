use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub id: String,
    pub name: String,
    pub accepting_crashes: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn product_cache_key(product_name: &str) -> String {
    format!("product:by-name:{product_name}")
}
