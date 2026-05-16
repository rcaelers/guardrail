use common::settings::ConfigError;
use serde::Deserialize;

use common::settings::{ObjectStorage, Valkey};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ProcessorServer {
    pub skip_patterns: Option<Vec<String>>,
    pub end_patterns: Option<Vec<String>>,
    pub delimiter: Option<String>,
    pub maximum_frame_count: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub processor: ProcessorServer,
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
}

impl Settings {
    pub fn load(config_dir: &str) -> Result<Self, ConfigError> {
        let s: Self = common::settings::load_settings(config_dir)?;
        Ok(s)
    }
}
