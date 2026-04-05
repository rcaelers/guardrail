use apalis_redis::{RedisConfig, RedisStorage};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::{Router, http::header::AUTHORIZATION};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::sync::Arc;
use std::{iter::once, net::SocketAddr, time::Duration};
use tower_http::{CompressionLevel, compression::CompressionLayer};
use tower_http::{
    decompression::RequestDecompressionLayer, sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;
use webauthn_rs::prelude::*;

use api::routes;
use api::state::AppState;
use api::worker::WorkQueue;
use common::jobs::queue;
use common::{init_logging, settings::Settings};
use repos::Repo;
const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

struct GuardrailApiApp {
    settings: Arc<Settings>,
}

impl GuardrailApiApp {
    async fn new(config_dir: &str) -> Self {
        Self {
            settings: Arc::new(
                Settings::with_config_dir(config_dir).expect("Failed to load settings"),
            ),
        }
    }

    async fn run(&self) {
        init_logging().await;

        info!("Starting server on port {}", self.settings.api_server.port);

        let guardrail_db = common::retry_startup("PostgreSQL", || async {
            self.init_guardrail_db().await
        })
        .await;
        let redis_conn = common::retry_startup("Valkey", || async {
            apalis_redis::connect(self.settings.valkey.uri.clone()).await
        })
        .await;
        let webauthn = self.create_webauthn();
        let repo = Repo::new(guardrail_db.clone());
        let store = common::init_s3_object_store(self.settings.clone()).await;

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let redis_symbol = RedisStorage::new_with_config(
            redis_conn.clone(),
            RedisConfig::new(queue::SYMBOL_JOBS),
        );
        let worker = Arc::new(WorkQueue::new(redis_symbol));

        let state = AppState {
            repo,
            webauthn,
            settings: self.settings.clone(),
            storage: store,
            worker,
        };

        let routes_all = Router::new()
            .nest("/api", routes::routes(state.clone()).await)
            .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
            .layer(RequestDecompressionLayer::new())
            .layer(CompressionLayer::new().quality(CompressionLevel::Fastest))
            .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
            .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(60)))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        if self.settings.api_server.public_key.is_some()
            && self.settings.api_server.private_key.is_some()
            && self
                .settings
                .api_server
                .public_key
                .clone()
                .unwrap_or_default()
                != ""
            && self
                .settings
                .api_server
                .private_key
                .clone()
                .unwrap_or_default()
                != ""
        {
            info!("Starting server with TLS");
            info!(
                "Public key: {}",
                self.settings
                    .api_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
            );
            info!(
                "Private key: {}",
                self.settings
                    .api_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
            );
            let config = RustlsConfig::from_pem(
                self.settings
                    .api_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                self.settings
                    .api_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = self.settings.clone().api_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind_rustls(addr, config)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        } else {
            let port = self.settings.clone().api_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        }
    }

    fn create_webauthn(&self) -> Arc<Webauthn> {
        let rp_id = self.settings.auth.id.as_str();
        let rp_origin = Url::parse(self.settings.auth.origin.as_str()).expect("Invalid URL");
        let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
        let builder = builder.rp_name(self.settings.auth.name.as_str());

        Arc::new(builder.build().expect("Invalid configuration"))
    }

    async fn init_guardrail_db(&self) -> Result<PgPool, sqlx::Error> {
        let database_url = &self.settings.database.db_uri;
        let mut opts: PgConnectOptions = database_url.parse()?;
        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Ok(pool)
    }
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let app = GuardrailApiApp::new(&args.config_dir).await;
    app.run().await;
}
