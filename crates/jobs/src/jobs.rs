use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinidumpJob {
    pub crash: serde_json::Value,
}
