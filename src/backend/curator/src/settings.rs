use common::settings::ConfigError;
use serde::Deserialize;

use common::settings::{Database, ObjectStorage, Valkey};

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
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
