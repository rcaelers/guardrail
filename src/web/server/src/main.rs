mod auth;
mod error;
mod oidc;
mod routes;
mod templates;
mod webauthn;

use std::{net::SocketAddr, sync::Arc};

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use common::{init_logging, retry_startup, settings::Settings};
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
    http_client: reqwest::Client,
    webauthn: Arc<Webauthn>,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let settings =
        Arc::new(Settings::with_config_dir(&args.config_dir).expect("Failed to load settings"));

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let db = retry_startup("SurrealDB", || {
        let settings = settings.clone();
        async move {
            let db = surrealdb::engine::any::connect(&settings.database.endpoint).await?;

            db.signin(surrealdb::opt::auth::Root {
                username: settings.database.username.clone(),
                password: settings.database.password.clone(),
            })
            .await?;

            db.use_ns(&settings.database.namespace)
                .use_db(&settings.database.database)
                .await?;

            Ok::<_, surrealdb::Error>(db)
        }
    })
    .await;

    let rp_id = settings.auth.id.clone();
    let rp_origin = Url::parse(&settings.auth.origin).expect("Invalid auth origin URL");
    let webauthn = Arc::new(
        WebauthnBuilder::new(&rp_id, &rp_origin)
            .expect("Failed to build Webauthn")
            .rp_name(&settings.auth.name)
            .build()
            .expect("Failed to build Webauthn"),
    );

    let state = AppState {
        repo: Repo::new(db),
        settings: settings.clone(),
        http_client: {
            let mut builder = reqwest::Client::builder();
            if settings
                .auth
                .oidc
                .as_ref()
                .and_then(|oidc| oidc.allow_insecure_tls)
                .unwrap_or(false)
            {
                builder = builder.danger_accept_invalid_certs(true);
            }
            builder.build().expect("Failed to build HTTP client")
        },
        webauthn,
    };

    let use_secure_cookies = settings
        .web_server
        .public_key
        .as_deref()
        .is_some_and(|pem| !pem.is_empty());

    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("guardrail")
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(4)))
        .with_secure(use_secure_cookies);

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
    if let (Some(public_key), Some(private_key)) =
        (settings.web_server.public_key.clone(), settings.web_server.private_key.clone())
        && !public_key.is_empty()
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
