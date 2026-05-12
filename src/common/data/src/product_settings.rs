use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EmailSettings {
    pub invite_html_template: Option<String>,
    pub invite_text_template: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductSettings {
    pub id: String,
    pub product_id: String,
    pub email: EmailSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
