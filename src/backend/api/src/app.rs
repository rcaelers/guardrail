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
use surrealdb::opt::auth::Root;
use tower_http::CompressionLevel;
use tower_http::compression::CompressionLayer;
use tower_http::decompression::RequestDecompressionLayer;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use common::jobs::queue;
use common::settings::Settings;
use repos::Repo;

use crate::routes;
use crate::state::AppState;
use crate::worker::WorkQueue;

pub const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

pub struct GuardrailApiApp {
    state: AppState,
}

impl GuardrailApiApp {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Bootstrap from settings: connect to SurrealDB, Valkey, S3, build internal state.
    pub async fn from_settings(settings: Arc<Settings>) -> Self {
        let db = surrealdb::engine::any::connect(&settings.database.endpoint)
            .await
            .expect("Failed to connect to SurrealDB");

        db.signin(Root {
            username: settings.database.username.clone(),
            password: settings.database.password.clone(),
        })
        .await
        .expect("Failed to sign in to SurrealDB");

        db.use_ns(&settings.database.namespace)
            .use_db(&settings.database.database)
            .await
            .expect("Failed to select namespace/database");

        let public_key = &settings.auth.jwk.public_key;
        db.query(format!(
            r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                WITH JWT ALGORITHM EDDSA KEY '{public_key}'
                DURATION FOR SESSION 1h"#
        ))
        .await
        .expect("Failed to define JWT access method");

        info!(
            "Connected to SurrealDB at {}",
            settings.database.endpoint
        );

        let redis_conn = apalis_redis::connect(settings.valkey.uri.clone())
            .await
            .expect("Failed to connect to Valkey (apalis)");

        let store = common::init_s3_object_store(settings.clone()).await;

        let repo = Repo::new(db);

        let redis_symbol = RedisStorage::new_with_config(
            redis_conn.clone(),
            RedisConfig::new(queue::SYMBOL_JOBS),
        );
        let worker = Arc::new(WorkQueue::new(redis_symbol));

        let state = AppState {
            repo,
            settings,
            storage: store,
            worker,
        };

        Self { state }
    }

    /// Access the repo for production bootstrap tasks (e.g. ensuring default API token).
    pub fn repo(&self) -> &Repo {
        &self.state.repo
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

        if settings.api_server.public_key.is_some()
            && settings.api_server.private_key.is_some()
            && settings.api_server.public_key.clone().unwrap_or_default() != ""
            && settings.api_server.private_key.clone().unwrap_or_default() != ""
        {
            info!("Starting server with TLS");
            info!("Public key: {}", settings.api_server.public_key.clone().unwrap_or_default());
            info!("Private key: {}", settings.api_server.private_key.clone().unwrap_or_default());
            let config = RustlsConfig::from_pem(
                settings
                    .api_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                settings
                    .api_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = settings.api_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind_rustls(addr, config)
                .serve(router.into_make_service())
                .await
                .unwrap();
        } else {
            let port = settings.api_server.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(router.into_make_service())
                .await
                .unwrap();
        }
    }
}
