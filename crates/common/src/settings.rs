use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Default)]
pub struct Server {
    pub port: u16,
    pub api_port: u16,
    pub base_path: String,
    pub site: String,
    pub max_minidump_size: Option<u64>,
    pub max_attachment_size: Option<u64>,
    pub max_symbols_size: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Auth {
    pub id: String,
    pub origin: String,
    pub name: String,
    pub jwk: Jwk,
    pub initial_admin_token: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Jwk {
    pub token_validity_in_minutes: i64,
    pub public_key: String,
    pub private_key: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Logger {
    pub directory: String,
    pub level: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Database {
    pub uri: String,
    pub name: String,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            uri: "xx".into(),
            name: "".into(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub server: Server,
    pub logger: Logger,
    pub database: Database,
    pub auth: Auth,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let builder = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name(&format!("config/{run_mode}")).required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(Environment::default().separator("__"));

        builder.build()?.try_deserialize()
    }
}
