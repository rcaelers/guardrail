use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Annotation {
    pub id: uuid::Uuid,
    pub key: String,
    pub kind: String,
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
    pub kind: String,
    pub value: String,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AnnotationKind {
    System,
    User,
}

impl AnnotationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnotationKind::System => "system",
            AnnotationKind::User => "user",
        }
    }
}

impl TryFrom<&str> for AnnotationKind {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "system" => Ok(AnnotationKind::System),
            "user" => Ok(AnnotationKind::User),
            _ => Err(format!("Invalid annotation kind: {s}")),
        }
    }
}

