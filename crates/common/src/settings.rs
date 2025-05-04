use config::{Config, ConfigError, File};
use glob::glob;
use natord::compare as natord_compare;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct ApiServer {
    pub port: u16,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct WebServer {
    pub port: u16,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Auth {
    pub id: String,
    pub origin: String,
    pub name: String,
    pub jwk: Jwk,
}

#[derive(Debug, Deserialize, Default)]
pub struct Jwk {
    pub token_validity_in_minutes: i64,
    pub public_key: String,
    pub private_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Database {
    pub uri: String,
}

impl Default for Database {
    fn default() -> Self {
        Self { uri: "xx".into() }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ObjectStorage {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub allow_http: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub api_server: ApiServer,
    pub web_server: WebServer,
    pub database: Database,
    pub object_storage: ObjectStorage,
    pub auth: Auth,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut files: Vec<_> = glob("config/*.yaml")
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
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(",")
                    .ignore_empty(true),
            );

        builder.build()?.try_deserialize()
    }
}
