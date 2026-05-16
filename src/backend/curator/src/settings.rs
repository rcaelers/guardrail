use common::settings::ConfigError;
use serde::Deserialize;

use common::settings::{Database, ObjectStorage, Valkey};

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
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
