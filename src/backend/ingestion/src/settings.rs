use common::settings::ConfigError;
use serde::Deserialize;
use std::fmt;

use common::settings::{ObjectStorage, Valkey};

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
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
}

impl Settings {
    pub fn load(config_dir: &str) -> Result<Self, ConfigError> {
        let s: Self = common::settings::load_settings(config_dir)?;
        Ok(s)
    }
}

#[cfg(test)]
impl Settings {
    pub fn test_default() -> Self {
        Self::default()
    }
}
