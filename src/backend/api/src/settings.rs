use common::settings::ConfigError;
use serde::Deserialize;
use std::fmt;

use common::settings::{Database, Jwk, ObjectStorage, Valkey};

#[derive(Deserialize, Default)]
pub struct ApiServer {
    pub port: u16,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

impl fmt::Debug for ApiServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiServer")
            .field("port", &self.port)
            .field("public_key", &self.public_key)
            .field("private_key", &self.private_key.as_deref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub api_server: ApiServer,
    pub jwk: Jwk,
    pub database: Database,
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
        s.jwk.public_key = testware::setup::TEST_PUBLIC_KEY.to_string();
        s.jwk.private_key = testware::setup::TEST_PRIVATE_KEY.to_string();
        s.config_dir = testware::workspace_config_dir();
        s
    }
}
