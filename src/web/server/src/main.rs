mod auth;
mod error;
mod routes;
mod templates;
mod webauthn;

use std::{net::SocketAddr, sync::Arc};

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use common::{init_logging, settings::Settings};
use repos::Repo;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::SameSite};
use tracing::info;
use webauthn_rs::prelude::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

#[derive(Clone)]
pub struct AppState {
    repo: Repo,
    settings: Arc<Settings>,
    webauthn: Arc<Webauthn>,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let settings =
        Arc::new(Settings::with_config_dir(&args.config_dir).expect("Failed to load settings"));

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let db = surrealdb::engine::any::connect(&settings.database.endpoint)
        .await
        .expect("Failed to connect to SurrealDB");

    db.signin(surrealdb::opt::auth::Root {
        username: settings.database.username.clone(),
        password: settings.database.password.clone(),
    })
    .await
    .expect("Failed to sign in to SurrealDB");

    db.use_ns(&settings.database.namespace)
        .use_db(&settings.database.database)
        .await
        .expect("Failed to select namespace/database");

    let rp_origin = Url::parse(settings.auth.origin.as_str()).expect("Invalid auth origin");
    let webauthn = Arc::new(
        WebauthnBuilder::new(settings.auth.id.as_str(), &rp_origin)
            .expect("Invalid WebAuthn configuration")
            .rp_name(settings.auth.name.as_str())
            .build()
            .expect("Invalid WebAuthn configuration"),
    );

    let state = AppState {
        repo: Repo::new(db),
        settings: settings.clone(),
        webauthn,
    };

    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("guardrail")
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(4)))
        .with_secure(false);

    let app = Router::new()
        .merge(routes::router())
        .nest_service("/static", ServeDir::new("src/web/server/static"))
        .route("/healthz", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(session_layer)
        .with_state(state);

    info!("Starting web server on port {}", settings.web_server.port);

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.web_server.port));
    if let (Some(public_key), Some(private_key)) = (
        settings.web_server.public_key.clone(),
        settings.web_server.private_key.clone(),
    ) && !public_key.is_empty()
        && !private_key.is_empty()
    {
        let tls = RustlsConfig::from_pem(public_key.into_bytes(), private_key.into_bytes())
            .await
            .expect("Failed to load TLS configuration");
        axum_server::bind_rustls(addr, tls)
            .serve(app.into_make_service())
            .await
            .expect("Failed to serve web app");
        return;
    }

    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .expect("Failed to serve web app");
}
