use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Product {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: String,
    pub accepting_crashes: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewProduct {
    pub name: String,
    pub description: String,
}

impl From<Product> for NewProduct {
    fn from(product: Product) -> Self {
        Self {
            name: product.name,
            description: product.description,
        }
    }
}
