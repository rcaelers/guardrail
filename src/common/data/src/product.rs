use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub public: bool,
    pub accepting_crashes: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewProduct {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

fn default_metadata() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

impl Default for NewProduct {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            public: false,
            metadata: default_metadata(),
        }
    }
}

impl From<Product> for NewProduct {
    fn from(product: Product) -> Self {
        Self {
            name: product.name,
            description: product.description,
            public: product.public,
            metadata: product.metadata,
        }
    }
}
