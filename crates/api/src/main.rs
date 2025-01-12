mod api;
mod app_state;
mod utils;

use axum::extract::DefaultBodyLimit;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use common::settings::settings;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing::{info, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, FmtSubscriber};

use app_state::AppState;

async fn init_logging() {
    let directory = &settings().logger.directory;

    let file_appender = tracing_appender::rolling::never(directory, "guardrail-api.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let max_level = settings().logger.level.parse().unwrap_or(Level::DEBUG);

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap()
        .add_directive("api=debug".parse().unwrap());

    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(filter)
        .finish()
        .with(fmt::Layer::new().with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

async fn init_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let connect_options = ConnectOptions::new(&settings().database.uri).to_owned();
    Database::connect(connect_options).await
}

#[tokio::main]
async fn main() {
    init_logging().await;

    info!("Starting server on port {}", settings().server.port);

    let db = init_db().await.unwrap();
    let state = AppState { db: db.clone() };

    let routes_all = Router::new()
        .nest("/api", api::routes().await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    //TODO: Make configurable
    let config = RustlsConfig::from_pem_file(
        PathBuf::from("dev").join("cert.pem"),
        PathBuf::from("dev").join("key.pem"),
    )
    .await
    .unwrap();

    let port = settings().server.port;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    axum_server::bind_rustls(addr, config)
        .serve(routes_all.into_make_service())
        .await
        .unwrap();
}
