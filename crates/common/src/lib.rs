pub mod settings;

#[cfg(feature = "ssr")]
pub mod token;

use serde::{Deserialize, Serialize};
use tracing_subscriber::{fmt::format::FmtSpan, layer::SubscriberExt, EnvFilter, FmtSubscriber};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
}

impl AuthenticatedUser {
    pub fn new(id: uuid::Uuid, username: String, is_admin: bool) -> Self {
        Self {
            id,
            username,
            is_admin,
        }
    }
}

use std::{collections::VecDeque, io::IsTerminal, ops::Range};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(String, SortOrder)>,
    pub range: Option<Range<usize>>,
    pub filter: Option<String>,
}

pub async fn init_logging() {
    // let pod = std::env::var("POD_NAME").unwrap_or_default();
    // let ns = std::env::var("POD_NAMESPACE").unwrap_or_default();

    let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_writer(std::io::stdout)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_current_span(false)
        .with_span_events(FmtSpan::CLOSE);

    let subscriber = FmtSubscriber::builder()
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(filter)
        .finish()
        .with(layer);

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing_log::LogTracer::init().expect("Failed to set logger");
}
