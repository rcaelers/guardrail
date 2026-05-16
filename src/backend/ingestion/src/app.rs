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
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing::info;

use common::jobs::queue;
use common::retry_startup;
use crate::settings::Settings;

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
        let redis_conn = retry_startup("Valkey (apalis)", || {
            let uri = settings.valkey.uri.clone();
            async move { apalis_redis::connect(uri).await }
        })
        .await;

        let store = common::init_s3_object_store(&settings.object_storage).await;

        let redis_client = redis::Client::open(settings.valkey.uri.as_str())
            .expect("Failed to create Redis client");
        let redis_manager = retry_startup("Valkey (redis)", || {
            let redis_client = redis_client.clone();
            async move { redis::aio::ConnectionManager::new(redis_client).await }
        })
        .await;
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
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .with_state(self.state.clone())
    }

    fn tls_configured(settings: &Settings) -> bool {
        settings
            .ingestion_server
            .public_key
            .as_deref()
            .is_some_and(|key| !key.is_empty())
            && settings
                .ingestion_server
                .private_key
                .as_deref()
                .is_some_and(|key| !key.is_empty())
    }

    pub async fn serve(&self) {
        let router = self.router().await;
        let settings = &self.state.settings;

        if Self::tls_configured(settings) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use object_store::memory::InMemory;
    use std::collections::HashMap;
    use tower::ServiceExt;

    use crate::worker::TestWorker;

    fn state() -> AppState {
        AppState {
            product_cache: ProductCache::from_map(HashMap::new()),
            settings: Arc::new(crate::settings::Settings::test_default()),
            storage: Arc::new(InMemory::new()),
            worker: Arc::new(TestWorker::new()),
        }
    }

    #[tokio::test]
    async fn new_and_router_wire_health_routes() {
        let app = GuardrailIngestionApp::new(state());
        let router = app.router().await;

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/minidump/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn tls_configured_requires_both_non_empty_keys() {
        let mut settings = crate::settings::Settings::test_default();
        settings.ingestion_server.public_key = None;
        settings.ingestion_server.private_key = None;
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingestion_server.public_key = Some("public".to_string());
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingestion_server.private_key = Some(String::new());
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingestion_server.private_key = Some("private".to_string());
        assert!(GuardrailIngestionApp::tls_configured(&settings));
    }
}
