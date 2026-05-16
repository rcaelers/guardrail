use common::settings::ConfigError;
use serde::Deserialize;

use common::settings::{Database, Ingress, Jwk, ObjectStorage, Valkey};

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub ingress: Ingress,
    pub jwk: Jwk,
    pub database: Database,
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
        let mut s = Self::default();
        s.jwk.public_key = testware::setup::TEST_PUBLIC_KEY.to_string();
        s.jwk.private_key = testware::setup::TEST_PRIVATE_KEY.to_string();
        s
    }
}
