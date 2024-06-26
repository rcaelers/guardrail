use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::{env, sync::OnceLock};

pub fn settings() -> &'static Settings {
    static INSTANCE: OnceLock<Settings> = OnceLock::new();
    INSTANCE.get_or_init(|| Settings::new().expect("Failed to setup settings"))
}

#[derive(Debug, Deserialize, Default)]
pub struct Server {
    pub port: u16,
    pub base_path: String,
    pub site: String,
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
    pub key: String,
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

#[derive(Debug, Deserialize)]
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
