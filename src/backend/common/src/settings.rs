use config::{Config, ConfigError, File};
use glob::glob;
use natord::compare as natord_compare;
use serde::Deserialize;

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ValidationScript {
    Global(String),
    ProductSpecific { product: String, script: String },
}

#[derive(Debug, Deserialize, Default)]
pub struct ApiServer {
    pub port: u16,
    #[serde(default = "default_true")]
    pub enable_webauthn: bool,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Minidumps {
    pub mandatory_annotations: Option<Vec<String>>,
    pub validation_scripts: Option<Vec<ValidationScript>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct IngestionServer {
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
pub struct Valkey {
    pub uri: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ProcessorServer {
    pub skip_patterns: Option<Vec<String>>,
    pub end_patterns: Option<Vec<String>>,
    pub delimiter: Option<String>,
    pub maximum_frame_count: Option<usize>,
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
    pub endpoint: String,
    pub namespace: String,
    pub database: String,
    pub username: String,
    pub password: String,
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
    pub ingestion_server: IngestionServer,
    pub web_server: WebServer,
    pub valkey: Valkey,
    pub processor: ProcessorServer,
    pub database: Database,
    pub object_storage: ObjectStorage,
    pub auth: Auth,
    pub minidumps: Minidumps,
    #[serde(skip)]
    pub config_dir: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        Self::with_config_dir("config")
    }

    pub fn with_config_dir(config_dir: &str) -> Result<Self, ConfigError> {
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
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(",")
                    .ignore_empty(true),
            );

        let mut settings: Self = builder.build()?.try_deserialize()?;
        settings.config_dir = config_dir.to_string();
        Ok(settings)
    }
}
