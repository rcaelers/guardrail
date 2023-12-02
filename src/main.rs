#![allow(dead_code, unused_variables)]
mod api;
mod app_state;
mod auth;
mod entity;
mod model;
mod session_store;
mod settings;
mod utils;
mod web;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::get_service;
use axum::{BoxError, Router};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::io::IsTerminal;
use std::sync::Arc;
use time::Duration;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::{info, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, FmtSubscriber};

use crate::auth::oidc::OidcClient;
use crate::session_store::SeaOrmSessionStore;
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

    let auth_client = Arc::new(OidcClient::new().await.unwrap());
    let state = Arc::new(AppState {
        db: db.clone(),
        auth_client,
    });

    let session_store = SeaOrmSessionStore::new(db);
    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(
            SessionManagerLayer::new(session_store)
                .with_name("guardrail")
                .with_same_site(SameSite::Lax)
                .with_expiry(Expiry::OnInactivity(Duration::hours(1)))
                .with_secure(false),
        );

    let routes_all = Router::new()
        .nest("/api", api::routes().await)
        .nest("/auth", auth::routes().await)
        .nest("/", web::routes(Arc::clone(&state)).await)
        .fallback_service(routes_static())
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(session_service)
        .with_state(state);

    let port = settings().server.port;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();
}

fn routes_static() -> Router {
    Router::new().nest_service("/", get_service(ServeDir::new("./web")))
}
