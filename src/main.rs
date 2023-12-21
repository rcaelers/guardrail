#![allow(dead_code, unused_variables)]
mod api;
mod app_state;
//mod auth;
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
use axum::response::IntoResponse;
use axum::{BoxError, Router};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::io::IsTerminal;
use std::sync::Arc;
use time::Duration;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::{info, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, FmtSubscriber};
use webauthn_rs::prelude::*;

//use crate::auth::oidc::OidcClient;
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

fn create_webauthn() -> Arc<Webauthn> {
    let rp_id = "localhost";
    let rp_origin = Url::parse("http://localhost:8080").expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let builder = builder.rp_name("Guardrail");

    Arc::new(builder.build().expect("Invalid configuration"))
}

#[tokio::main]
async fn main() {
    init_logging().await;

    info!("Starting server on port {}", settings().server.port);

    let db = init_db().await.unwrap();
    let webauthn = create_webauthn();
    let state = Arc::new(AppState {
        db: db.clone(),
        webauthn,
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
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .nest("/api", api::routes().await)
        .nest("/auth", auth::routes(Arc::clone(&state)).await)
        .nest("/", web::routes(Arc::clone(&state)).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(session_service)
        .with_state(state)
        .fallback(handler_404);

    let port = settings().server.port;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
