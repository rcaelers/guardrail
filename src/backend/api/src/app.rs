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
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing::info;

use common::jobs::queue;
use common::retry_startup;
use crate::settings::Settings;
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
        let db = retry_startup("SurrealDB", || {
            let settings = settings.clone();
            async move {
                let db = surrealdb::engine::any::connect(&settings.database.endpoint).await?;

                db.signin(Root {
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

        let public_key = &settings.jwk.public_key;
        db.query(format!(
            r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                WITH JWT ALGORITHM EDDSA KEY '{public_key}'
                AUTHENTICATE {{
                    IF $auth.id {{
                        RETURN $auth.id;
                    }};
                    IF $token.user_id {{
                        RETURN type::record('users', $token.user_id);
                    }};
                    IF $token.username {{
                        RETURN type::record('users', $token.username);
                    }};
                }}
                DURATION FOR SESSION 1h"#
        ))
        .await
        .expect("Failed to define JWT access method");

        info!("Connected to SurrealDB at {}", settings.database.endpoint);

        let redis_conn = retry_startup("Valkey (apalis)", || {
            let uri = settings.valkey.uri.clone();
            async move { apalis_redis::connect(uri).await }
        })
        .await;

        let store = common::init_s3_object_store(&settings.object_storage).await;

        let repo = Repo::new(db);

        let redis_symbol =
            RedisStorage::new_with_config(redis_conn.clone(), RedisConfig::new(queue::SYMBOL_JOBS));
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
            info!("Starting server with TLS");
            info!("Public key: {}", settings.ingress.public_key.clone().unwrap_or_default());
            info!("Private key: {}", settings.ingress.private_key.clone().unwrap_or_default());
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
                .serve(router.into_make_service())
                .await
                .unwrap();
        } else {
            let port = settings.ingress.port;
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            axum_server::bind(addr)
                .serve(router.into_make_service())
                .await
                .unwrap();
        }
    }

    pub async fn ensure_default_api_token(&self) -> Result<(), Box<dyn std::error::Error>> {
        use common::token::generate_api_token;
        use data::api_token::NewApiToken;
        use repos::api_token::ApiTokenRepo;

        let tokens = ApiTokenRepo::get_all(&self.state.repo.db).await?;
        if !tokens.is_empty() {
            info!("API tokens already exist, skipping default token creation");
            return Ok(());
        }

        let (token_id, token, token_hash) =
            generate_api_token().map_err(|_| "Failed to generate API token")?;

        let new_token = NewApiToken {
            description: "Default API token".to_string(),
            token_id,
            token_hash,
            product_id: None,
            user_id: None,
            entitlements: vec!["token".to_string()],
            expires_at: None,
            is_active: true,
        };

        let _token_id = ApiTokenRepo::create(&self.state.repo.db, new_token).await?;
        info!("Created default API token");

        if let Err(err) = Self::create_k8s_initial_token_secret(&token).await {
            tracing::warn!("Failed to create initial token secret: {}", err);
        }

        Ok(())
    }

    async fn create_k8s_initial_token_secret(
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use k8s_openapi::api::core::v1::Secret;
        use kube::{
            Api, Client,
            api::{ObjectMeta, PostParams},
        };

        const SECRET_NAME: &str = "guardrail-initial-admin-token";

        let client = Client::try_default().await?;
        let namespace =
            std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
                .unwrap_or_else(|_| {
                    tracing::warn!("Could not determine current namespace, using 'default'");
                    "default".to_string()
                });

        let secrets: Api<Secret> = Api::namespaced(client, &namespace);

        if secrets.get_opt(SECRET_NAME).await?.is_some() {
            return Ok(());
        }

        let secret = Secret {
            metadata: ObjectMeta {
                name: Some(SECRET_NAME.to_string()),
                labels: Some(
                    [("app.kubernetes.io/part-of".to_string(), "guardrail".to_string())].into(),
                ),
                ..Default::default()
            },
            string_data: Some([("token".to_string(), token.to_string())].into()),
            type_: Some("Opaque".to_string()),
            ..Default::default()
        };

        secrets.create(&PostParams::default(), &secret).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use object_store::memory::InMemory;
    use tower::ServiceExt;

    use crate::worker::TestWorker;

    async fn state() -> AppState {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        AppState {
            repo: Repo::new(db),
            settings: Arc::new(crate::settings::Settings::default()),
            storage: Arc::new(InMemory::new()),
            worker: Arc::new(TestWorker::new()),
        }
    }

    #[test]
    fn tls_configured_requires_both_non_empty_keys() {
        let mut settings = crate::settings::Settings::default();
        settings.ingress.public_key = None;
        settings.ingress.private_key = None;
        assert!(!GuardrailApiApp::tls_configured(&settings));

        settings.ingress.public_key = Some("public".to_string());
        assert!(!GuardrailApiApp::tls_configured(&settings));

        settings.ingress.private_key = Some(String::new());
        assert!(!GuardrailApiApp::tls_configured(&settings));

        settings.ingress.private_key = Some("private".to_string());
        assert!(GuardrailApiApp::tls_configured(&settings));
    }

    #[tokio::test]
    async fn new_repo_and_router_wire_health_routes() {
        let state = state().await;
        let app = GuardrailApiApp::new(state.clone());
        assert_eq!(app.repo().db.health().await, Ok(()));

        let router = app.router().await;
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
