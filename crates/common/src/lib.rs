pub mod settings;

#[cfg(feature = "ssr")]
pub mod token;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
}

impl AuthenticatedUser {
    pub fn new(id: uuid::Uuid, username: String, is_admin: bool) -> Self {
        Self {
            id,
            username,
            is_admin,
        }
    }
}

use std::{collections::VecDeque, ops::Range};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(String, SortOrder)>,
    pub range: Option<Range<usize>>,
    pub filter: Option<String>,
}
