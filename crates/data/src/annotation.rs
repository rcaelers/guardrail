use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Annotation {
    pub id: uuid::Uuid,
    pub key: String,
    pub source: String,
    pub value: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewAnnotation {
    pub key: String,
    pub source: String,
    pub value: String,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AnnotationSource {
    Submission,
    Script,
    User,
}

impl AnnotationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnotationSource::Submission => "submission",
            AnnotationSource::Script => "script",
            AnnotationSource::User => "user",
        }
    }
}

impl TryFrom<&str> for AnnotationSource {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "submission" => Ok(AnnotationSource::Submission),
            "script" => Ok(AnnotationSource::Script),
            "user" => Ok(AnnotationSource::User),
            _ => Err(format!("Invalid annotation source: {s}")),
        }
    }
}
