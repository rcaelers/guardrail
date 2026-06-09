use common::settings::ConfigError;
use serde::Deserialize;

use common::settings::{Ingress, ObjectStorage, Valkey};

/// Abuse protection for the public crash-upload endpoint. Counters are kept in
/// Valkey so limits hold across replicas.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct RateLimit {
    pub enabled: bool,
    /// Max uploads per client IP per minute (0 disables the per-IP limit).
    pub per_ip_per_minute: u64,
    /// Max uploads per product token per minute (0 disables the per-token limit).
    pub per_token_per_minute: u64,
    /// Trust the left-most `X-Forwarded-For` entry for the client IP. Enable only
    /// when running behind a proxy/ingress that overwrites the header, otherwise
    /// clients can spoof it to evade the per-IP limit.
    pub trust_forwarded_for: bool,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            enabled: true,
            per_ip_per_minute: 120,
            per_token_per_minute: 600,
            trust_forwarded_for: true,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub ingress: Ingress,
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
    #[serde(default)]
    pub rate_limit: RateLimit,
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
