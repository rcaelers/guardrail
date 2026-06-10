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
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing::info;

use crate::settings::Settings;
use common::jobs::queue;
use common::retry_startup;

use crate::product_cache::ProductCache;
use crate::rate_limit::RateLimiter;
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
        let rate_limiter = Some(Arc::new(RateLimiter::new(
            redis_manager.clone(),
            settings.rate_limit.clone(),
        )));
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
            rate_limiter,
        };

        Self { state }
    }

    pub async fn router(&self) -> Router {
        Router::new()
            .nest("/api", routes::routes(self.state.clone()).await)
            .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
            // Bound the *decompressed* request body. This layer sits inner to
            // RequestDecompressionLayer, so it counts post-decompression bytes
            // and stops a small `Content-Encoding: gzip` bomb from expanding into
            // an unbounded stream to S3. axum's DefaultBodyLimit below caps the
            // raw/compressed body but does not reliably bound streaming Multipart.
            .layer(RequestBodyLimitLayer::new(MAX_UPLOAD_BYTES))
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
            .ingress
            .public_key
            .as_deref()
            .is_some_and(|key| !key.is_empty())
            && settings
                .ingress
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
                    .ingress
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                settings
                    .ingress
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = settings.ingress.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind_rustls(addr, config)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .unwrap();
        } else {
            let port = settings.ingress.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
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
            rate_limiter: None,
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
        settings.ingress.public_key = None;
        settings.ingress.private_key = None;
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingress.public_key = Some("public".to_string());
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingress.private_key = Some(String::new());
        assert!(!GuardrailIngestionApp::tls_configured(&settings));

        settings.ingress.private_key = Some("private".to_string());
        assert!(GuardrailIngestionApp::tls_configured(&settings));
    }

    // A small `Content-Encoding: gzip` body that inflates past MAX_UPLOAD_BYTES
    // must be rejected, not streamed unbounded to storage. Guards the layer order
    // (RequestBodyLimitLayer inner to RequestDecompressionLayer).
    #[tokio::test]
    async fn decompression_bomb_is_rejected() {
        use axum::http::header::{CONTENT_ENCODING, CONTENT_TYPE};
        use common::product_info::ProductInfo;
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let token = "decompression_bomb_test_token_0001";
        let product = ProductInfo {
            id: "product-1".to_string(),
            name: "TestProduct".to_string(),
            accepting_crashes: true,
            metadata: serde_json::json!({}),
            mandatory_annotations: vec![],
            validation_scripts: vec![],
            processor_settings: None,
        };
        let state = AppState {
            product_cache: ProductCache::from_token_map(HashMap::from([(
                token.to_string(),
                product,
            )])),
            settings: Arc::new(crate::settings::Settings::test_default()),
            storage: Arc::new(InMemory::new()),
            worker: Arc::new(TestWorker::new()),
            rate_limiter: None,
        };
        let router = GuardrailIngestionApp::new(state).router().await;

        // Multipart whose minidump field decompresses to well over the limit.
        let boundary = "----guardrail-bomb-boundary";
        let filler = vec![b'0'; MAX_UPLOAD_BYTES + 10 * 1024 * 1024];
        let mut plain = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; \
             filename=\"x.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        plain.extend_from_slice(&filler);
        plain.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&plain).unwrap();
        let compressed = encoder.finish().unwrap();
        // The compressed payload itself is comfortably under the limit.
        assert!(compressed.len() < MAX_UPLOAD_BYTES);

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/minidump/{token}/upload"))
                    .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
                    .header(CONTENT_ENCODING, "gzip")
                    .body(Body::from(compressed))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::OK);
        assert!(
            response.status().is_client_error() || response.status().is_server_error(),
            "bomb should be rejected, got {}",
            response.status()
        );
    }
}
