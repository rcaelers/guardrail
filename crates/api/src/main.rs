use apalis_sql::Config;
use apalis_sql::postgres::PostgresStorage;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum_server::tls_rustls::RustlsConfig;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    Api, Client,
    api::{ObjectMeta, PostParams},
};
use object_store::{ObjectStore, aws::AmazonS3Builder};
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;
use webauthn_rs::prelude::*;

use api::routes;
use api::state::AppState;
use api::worker::MinidumpProcessor;
use common::token::generate_api_token;
use common::{init_logging, settings::Settings};
use repos::Repo;

const SECRET_NAME: &str = "guardrail-initial-admin-token";

struct GuardrailApp {
    settings: Arc<Settings>,
}

impl GuardrailApp {
    async fn new() -> Self {
        Self {
            settings: Arc::new(Settings::new().expect("Failed to load settings")),
        }
    }

    async fn run(&self) {
        init_logging().await;

        let settings = Arc::new(Settings::new().expect("Failed to load settings"));
        info!("Starting server on port {}", settings.clone().api_server.port);

        let db = self.init_db().await.unwrap();
        let webauthn = self.create_webauthn();
        let repo = Repo::new(db.clone());
        let store = self.init_s3_object_store().await;

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        if let Err(err) = self.ensure_default_api_token(&repo).await {
            tracing::error!("Failed to create default API token: {}", err);
        }

        let pg = PostgresStorage::new_with_config(db.clone(), Config::new("guardrail::Jobs"));
        let worker = Arc::new(MinidumpProcessor::new(pg.clone()));

        let state = AppState {
            repo,
            webauthn,
            settings: settings.clone(),
            storage: store,
            worker,
        };

        let routes_all = Router::new()
            .nest("/api", routes::routes(state.clone()).await)
            .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        if self.settings.api_server.public_key.is_some()
            && self.settings.api_server.private_key.is_some()
        {
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
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            axum_server::bind_rustls(addr, config)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        } else {
            let port = self.settings.clone().api_server.port;
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
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

    async fn init_db(&self) -> Result<PgPool, sqlx::Error> {
        let database_url = &self.settings.database.uri;
        let mut opts: PgConnectOptions = database_url.parse()?;
        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Ok(pool)
    }

    async fn init_s3_object_store(&self) -> Arc<dyn ObjectStore> {
        let storage_config = &self.settings.object_storage;

        let mut builder = AmazonS3Builder::from_env();

        if let Some(region) = &storage_config.region {
            builder = builder.with_region(region);
        }

        if let Some(endpoint) = &storage_config.endpoint {
            builder = builder.with_endpoint(endpoint);
        }

        if let Some(allow_http) = storage_config.allow_http {
            builder = builder.with_allow_http(allow_http);
        }

        if let Some(access_key_id) = &storage_config.access_key_id {
            builder = builder.with_access_key_id(access_key_id);
        }

        if let Some(secret_access_key) = &storage_config.secret_access_key {
            builder = builder.with_secret_access_key(secret_access_key);
        }

        let store = builder
            .with_bucket_name(storage_config.bucket.clone())
            .build()
            .expect("failed to build AmazonS3");

        Arc::new(store)
    }

    async fn create_k8s_initial_token_secret(
        &self,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        secrets
            .create(&PostParams::default(), &secret)
            .await
            .expect("Failed to create secret");
        Ok(())
    }

    async fn ensure_default_api_token(
        &self,
        repo: &Repo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use data::api_token::NewApiToken;
        use repos::api_token::ApiTokenRepo;
        use tracing::info;

        let mut conn = repo.acquire_admin().await?;

        let tokens = ApiTokenRepo::get_all(&mut *conn).await?;
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

        let _token_id = ApiTokenRepo::create(&mut *conn, new_token).await?;
        info!("Created default API token: {}", token);

        self.create_k8s_initial_token_secret(&token).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let app = GuardrailApp::new();
    app.await.run().await;
}
