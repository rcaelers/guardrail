use apalis_redis::{RedisConfig, RedisStorage};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::{Router, http::header::AUTHORIZATION};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use std::sync::Arc;
use std::{iter::once, net::SocketAddr, time::Duration};
use tower_http::{CompressionLevel, compression::CompressionLayer};
use tower_http::{
    decompression::RequestDecompressionLayer, sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;

use ingestion::product_cache::ProductCache;
use ingestion::routes;
use ingestion::state::AppState;
use ingestion::worker::WorkQueue;
use common::jobs::queue;
use common::{init_logging, settings::Settings};

const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

struct GuardrailIngestionApp {
    settings: Arc<Settings>,
}

impl GuardrailIngestionApp {
    async fn new(config_dir: &str) -> Self {
        Self {
            settings: Arc::new(
                Settings::with_config_dir(config_dir).expect("Failed to load settings"),
            ),
        }
    }

    async fn run(&self) {
        init_logging().await;

        info!(
            "Starting ingestion server on port {}",
            self.settings.ingestion_server.port
        );

        let redis_conn = apalis_redis::connect(self.settings.job_server.redis_uri.clone())
            .await
            .expect("Failed to connect to Redis/Valkey");
        let store = common::init_s3_object_store(self.settings.clone()).await;

        let redis_client =
            redis::Client::open(self.settings.job_server.redis_uri.as_str())
                .expect("Failed to create Redis client");
        let redis_manager = redis::aio::ConnectionManager::new(redis_client)
            .await
            .expect("Failed to create Redis connection manager");
        let product_cache = ProductCache::new(redis_manager);

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let redis_minidump = RedisStorage::new_with_config(
            redis_conn.clone(),
            RedisConfig::new(queue::MINIDUMP_JOBS),
        );
        let worker = Arc::new(WorkQueue::new(redis_minidump));

        let state = AppState {
            product_cache,
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
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(60),
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        if self.settings.ingestion_server.public_key.is_some()
            && self.settings.ingestion_server.private_key.is_some()
            && self
                .settings
                .ingestion_server
                .public_key
                .clone()
                .unwrap_or_default()
                != ""
            && self
                .settings
                .ingestion_server
                .private_key
                .clone()
                .unwrap_or_default()
                != ""
        {
            info!("Starting ingestion server with TLS");
            let config = RustlsConfig::from_pem(
                self.settings
                    .ingestion_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                self.settings
                    .ingestion_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = self.settings.ingestion_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind_rustls(addr, config)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        } else {
            let port = self.settings.ingestion_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let app = GuardrailIngestionApp::new(&args.config_dir).await;
    app.run().await;
}
