use common::settings::ConfigError;
use serde::Deserialize;
use std::fmt;

use common::settings::{Database, Ingress, ObjectStorage, Oidc, Valkey};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ResendSettings {
    pub key: String,
}

impl fmt::Debug for ResendSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResendSettings")
            .field(
                "key",
                &if self.key.is_empty() {
                    "[not set]"
                } else {
                    "[REDACTED]"
                },
            )
            .finish()
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EmailSettings {
    pub from: String,
    pub resend: Option<ResendSettings>,
}

impl fmt::Debug for EmailSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmailSettings")
            .field("from", &self.from)
            .field("resend", &self.resend)
            .finish()
    }
}

#[derive(Deserialize, Default)]
pub struct PocketIdSettings {
    pub api_url: String,
    pub api_key: String,
    pub public_url: Option<String>,
    pub setup_path: Option<String>,
    pub post_setup_redirect: Option<String>,
    /// URL of the Guardrail auto-login page served at the PocketID domain.
    /// When set, overrides setup_path/post_setup_redirect for the invite flow.
    pub auto_login_url: Option<String>,
}

impl fmt::Debug for PocketIdSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PocketIdSettings")
            .field("api_url", &self.api_url)
            .field("api_key", &"[REDACTED]")
            .field("public_url", &self.public_url)
            .field("setup_path", &self.setup_path)
            .finish()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ProvisionerSettings {
    pub pocket_id: Option<PocketIdSettings>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ProcessorDefaults {
    pub skip_patterns: Vec<String>,
    pub end_patterns: Vec<String>,
    pub delimiter: Option<String>,
    pub maximum_frame_count: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub ingress: Ingress,
    pub oidc: Option<Oidc>,
    pub database: Database,
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
    #[serde(default)]
    pub provisioner: ProvisionerSettings,
    #[serde(default)]
    pub email: EmailSettings,
    #[serde(default)]
    pub processor: ProcessorDefaults,
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
        s.database.jwk.public_key = testware::setup::TEST_PUBLIC_KEY.to_string();
        s.database.jwk.private_key = testware::setup::TEST_PRIVATE_KEY.to_string();
        s.database.namespace = "test".to_string();
        s.database.database = "test".to_string();
        s
    }
}
