use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EmailSettings {
    pub invite_subject: Option<String>,
    pub invite_html_template: Option<String>,
    pub invite_text_template: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProcessorSettings {
    pub skip_patterns: Option<Vec<String>>,
    pub end_patterns: Option<Vec<String>>,
    pub delimiter: Option<String>,
    pub maximum_frame_count: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MinidumpSettings {
    pub mandatory_annotations: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductSettings {
    pub id: String,
    pub product_id: String,
    pub email: EmailSettings,
    #[serde(default)]
    pub processor: ProcessorSettings,
    #[serde(default)]
    pub minidump: MinidumpSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
