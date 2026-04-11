use apalis_redis::{RedisConfig, RedisStorage};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum_server::tls_rustls::RustlsConfig;
use std::iter::once;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::CompressionLevel;
use tower_http::compression::CompressionLayer;
use tower_http::decompression::RequestDecompressionLayer;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use common::jobs::queue;
use common::settings::Settings;

use crate::product_cache::ProductCache;
use crate::routes;
use crate::state::AppState;
use crate::worker::WorkQueue;

pub const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

pub struct GuardrailIngestionApp {
    state: AppState,
}

impl GuardrailIngestionApp {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Bootstrap from settings: connect to Valkey and S3, build internal state.
    pub async fn from_settings(settings: Arc<Settings>) -> Self {
        let redis_conn = apalis_redis::connect(settings.valkey.uri.clone())
            .await
            .expect("Failed to connect to Valkey (apalis)");

        let store = common::init_s3_object_store(settings.clone()).await;

        let redis_client = redis::Client::open(settings.valkey.uri.as_str())
            .expect("Failed to create Redis client");
        let redis_manager = redis::aio::ConnectionManager::new(redis_client)
            .await
            .expect("Failed to create Redis connection manager");
        let product_cache = ProductCache::new(redis_manager);

        let redis_minidump = RedisStorage::new_with_config(
            redis_conn.clone(),
            RedisConfig::new(queue::MINIDUMP_JOBS),
        );
        let worker = Arc::new(WorkQueue::new(redis_minidump));

        let state = AppState {
            product_cache,
            settings,
            storage: store,
            worker,
        };

        Self { state }
    }

    pub async fn router(&self) -> Router {
        Router::new()
            .nest("/api", routes::routes(self.state.clone()).await)
            .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
            .layer(RequestDecompressionLayer::new())
            .layer(CompressionLayer::new().quality(CompressionLevel::Fastest))
            .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(60),
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone())
    }

    pub async fn serve(&self) {
        let router = self.router().await;
        let settings = &self.state.settings;

        if settings.ingestion_server.public_key.is_some()
            && settings.ingestion_server.private_key.is_some()
            && settings
                .ingestion_server
                .public_key
                .clone()
                .unwrap_or_default()
                != ""
            && settings
                .ingestion_server
                .private_key
                .clone()
                .unwrap_or_default()
                != ""
        {
            info!("Starting ingestion server with TLS");
            let config = RustlsConfig::from_pem(
                settings
                    .ingestion_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                settings
                    .ingestion_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = settings.ingestion_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind_rustls(addr, config)
                .serve(router.into_make_service())
                .await
                .unwrap();
        } else {
            let port = settings.ingestion_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(router.into_make_service())
                .await
                .unwrap();
        }
    }
}
