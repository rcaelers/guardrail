pub use config::ConfigError;
use config::{Config, File};
use glob::glob;
use natord::compare as natord_compare;
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Deserialize, Default)]
pub struct Valkey {
    pub uri: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Oidc {
    pub issuer_url: String,
    pub internal_issuer_url: Option<String>,
    pub client_id: String,
    pub client_secret: String,
    pub callback_url: String,
    pub logout_callback_url: String,
    pub launch_url: Option<String>,
    pub self_service_url: Option<String>,
    pub pkce: Option<bool>,
    pub allow_insecure_tls: Option<bool>,
}

impl fmt::Debug for Oidc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Oidc")
            .field("issuer_url", &self.issuer_url)
            .field("internal_issuer_url", &self.internal_issuer_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("callback_url", &self.callback_url)
            .field("logout_callback_url", &self.logout_callback_url)
            .field("launch_url", &self.launch_url)
            .field("self_service_url", &self.self_service_url)
            .field("pkce", &self.pkce)
            .field("allow_insecure_tls", &self.allow_insecure_tls)
            .finish()
    }
}

#[derive(Deserialize, Default)]
pub struct Jwk {
    pub token_validity_in_minutes: i64,
    pub public_key: String,
    pub private_key: String,
}

impl fmt::Debug for Jwk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Jwk")
            .field("token_validity_in_minutes", &self.token_validity_in_minutes)
            .field("public_key", &self.public_key)
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Database {
    pub endpoint: String,
    pub namespace: String,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("endpoint", &self.endpoint)
            .field("namespace", &self.namespace)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

impl Default for Database {
    fn default() -> Self {
        Self {
            endpoint: "ws://localhost:8000".into(),
            namespace: "guardrail".into(),
            database: "guardrail".into(),
            username: "root".into(),
            password: "root".into(),
        }
    }
}

#[derive(Deserialize, Default)]
pub struct ObjectStorage {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub allow_http: Option<bool>,
}

impl fmt::Debug for ObjectStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectStorage")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint", &self.endpoint)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &self.secret_access_key.as_deref().map(|_| "[REDACTED]"))
            .field("allow_http", &self.allow_http)
            .finish()
    }
}

pub fn load_settings<T: serde::de::DeserializeOwned + Default>(
    config_dir: &str,
) -> Result<T, ConfigError> {
    let pattern = format!("{config_dir}/*.yaml");
    let mut files: Vec<_> = glob(&pattern)
        .expect("Failed to read config files")
        .filter_map(|entry| entry.ok())
        .collect();

    files.sort_by(|a, b| {
        natord_compare(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });

    let builder = Config::builder()
        .add_source(files.into_iter().map(File::from).collect::<Vec<_>>())
        .add_source(
            config::Environment::with_prefix("GUARDRAIL")
                .separator("_")
                .ignore_empty(true),
        );

    builder.build()?.try_deserialize()
}
