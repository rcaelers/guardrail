use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct ApiToken {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub description: String,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub last_used_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewApiToken {
    pub description: String,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub expires_at: Option<NaiveDateTime>,
}
