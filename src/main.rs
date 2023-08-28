#![allow(dead_code, unused_variables)]
mod api;
mod app_state;
mod entity;
mod model;
mod settings;
mod utils;

use axum::extract::DefaultBodyLimit;
use axum::routing::get_service;
use axum::Router;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing::{info, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, FmtSubscriber};

use app_state::AppState;
use settings::settings;

async fn init_logging() {
    let directory = &settings().logger.directory;

    let file_appender = tracing_appender::rolling::never(directory, "guardrail.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let max_level = settings().logger.level.parse().unwrap_or(Level::DEBUG);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_ansi(std::io::stdout().is_terminal())
        .finish()
        .with(fmt::Layer::new().with_writer(non_blocking));
    // .with(fmt::Layer::new().with_writer(std::io::stdout));

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
    let state = Arc::new(AppState { db });

    let routes_all = Router::new()
        .nest("/api", api::routes())
        .fallback_service(routes_static())
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(state);

    let port = settings().server.port;
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    axum::Server::bind(&address)
        .serve(routes_all.into_make_service())
        .await
        .unwrap();
}

fn routes_static() -> Router {
    Router::new().nest_service("/", get_service(ServeDir::new("./web")))
}
