use common::settings::ConfigError;
use serde::Deserialize;
use std::fmt;

use common::settings::{Auth, Database, ObjectStorage, Valkey};

#[derive(Deserialize, Default)]
pub struct WebServer {
    pub port: u16,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

impl fmt::Debug for WebServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebServer")
            .field("port", &self.port)
            .field("public_key", &self.public_key)
            .field("private_key", &self.private_key.as_deref().map(|_| "[REDACTED]"))
            .finish()
    }
}

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
pub struct Settings {
    pub web_server: WebServer,
    pub base_url: String,
    pub auth: Auth,
    pub database: Database,
    pub valkey: Valkey,
    pub object_storage: ObjectStorage,
    #[serde(default)]
    pub provisioner: ProvisionerSettings,
    #[serde(default)]
    pub email: EmailSettings,
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
        s.auth.jwk.public_key = testware::setup::TEST_PUBLIC_KEY.to_string();
        s.auth.jwk.private_key = testware::setup::TEST_PRIVATE_KEY.to_string();
        s.database.namespace = "test".to_string();
        s.database.database = "test".to_string();
        s.config_dir = testware::workspace_config_dir();
        s
    }
}
