use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppEmailSettings {
    pub recovery_subject: Option<String>,
    pub recovery_html_template: Option<String>,
    pub recovery_text_template: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppSettings {
    pub id: String,
    pub email: AppEmailSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
