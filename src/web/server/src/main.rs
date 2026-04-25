mod auth;
mod db_api;
mod error;
mod invite;
mod oidc;
mod pocket_id;
mod provisioner;
mod routes;
mod templates;
mod webauthn;

use std::{net::SocketAddr, sync::Arc};

use provisioner::IdentityProvisioner;

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use common::{init_logging, retry_startup, settings::Settings};
use repos::Repo;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::SameSite};
use tracing::Level;
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
    provisioner: Option<Arc<dyn IdentityProvisioner>>,
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

    let http_client = {
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
    };

    let provisioner: Option<Arc<dyn IdentityProvisioner>> =
        settings.provisioner.pocket_id.as_ref().map(|cfg| {
            let setup_path = cfg
                .setup_path
                .clone()
                .unwrap_or_else(|| "/one-time-access".to_string());
            Arc::new(pocket_id::PocketIdProvisioner {
                base_url: url::Url::parse(&cfg.api_url)
                    .expect("Invalid provisioner.pocket_id.api_url"),
                api_key: cfg.api_key.clone(),
                setup_path,
                client: http_client.clone(),
            }) as Arc<dyn IdentityProvisioner>
        });

    let state = AppState {
        repo: Repo::new(db),
        settings: settings.clone(),
        http_client,
        webauthn,
        provisioner,
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

    let storage = common::init_s3_object_store(settings.clone()).await;
    let db_state = db_api::DbState {
        db: Arc::new(state.repo.db.clone()),
        storage,
    };

    let app = Router::new()
        .merge(routes::router())
        .nest("/api/v1", db_api::router().with_state(db_state))
        .nest_service("/static", ServeDir::new("src/web/server/static"))
        .route("/healthz", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
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
