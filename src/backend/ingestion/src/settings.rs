use common::settings::ConfigError;
use serde::Deserialize;
use std::fmt;

use common::settings::{ObjectStorage, Valkey};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ValidationScript {
    Global(String),
    ProductSpecific { product: String, script: String },
}

#[derive(Debug, Deserialize, Default)]
pub struct Minidumps {
    pub mandatory_annotations: Option<Vec<String>>,
    pub validation_scripts: Option<Vec<ValidationScript>>,
}

#[derive(Deserialize, Default)]
pub struct IngestionServer {
    pub port: u16,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

impl fmt::Debug for IngestionServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IngestionServer")
            .field("port", &self.port)
            .field("public_key", &self.public_key)
            .field("private_key", &self.private_key.as_deref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub ingestion_server: IngestionServer,
    pub minidumps: Minidumps,
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
    #[serde(skip)]
    pub config_dir: String,
}

impl Settings {
    pub fn load(config_dir: &str) -> Result<Self, ConfigError> {
        let mut s: Self = common::settings::load_settings(config_dir)?;
        s.config_dir = config_dir.to_string();
        Ok(s)
    }
}

#[cfg(test)]
impl Settings {
    pub fn test_default() -> Self {
        let mut s = Self::default();
        s.minidumps.mandatory_annotations =
            Some(vec!["product".to_string(), "version".to_string()]);
        s.minidumps.validation_scripts = Some(vec![
            ValidationScript::Global("scripts/product_validation.rhai".to_string()),
            ValidationScript::Global("scripts/build_age_validation.rhai".to_string()),
        ]);
        s.config_dir = testware::workspace_config_dir();
        s
    }
}
