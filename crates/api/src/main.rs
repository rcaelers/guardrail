use api::routes;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum_server::tls_rustls::RustlsConfig;
use common::hash_token;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing::{Level, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, FmtSubscriber, fmt};
use webauthn_rs::prelude::*;

use api::state::AppState;
use common::settings::Settings;
use repos::Repo;

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
        self.init_logging().await;

        let settings = Arc::new(Settings::new().expect("Failed to load settings"));
        info!("Starting server on port {}", settings.clone().server.api_port);

        let db = self.init_db().await.unwrap();
        let webauthn = self.create_webauthn();
        let repo = Repo::new(db.clone());
        let store = Arc::new(
            object_store::aws::AmazonS3Builder::from_env()
                .with_url(settings.clone().server.store.clone())
                .build()
                .expect("Failed to create object store"),
        );

        if let Err(err) = self.ensure_default_api_token(&repo).await {
            tracing::error!("Failed to create default API token: {}", err);
        }
        let state = AppState {
            repo,
            webauthn,
            settings: settings.clone(),
            storage: store,
        };

        let routes_all = Router::new()
            .nest("/api", routes::routes(state.clone()).await)
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        //TODO: Make configurable
        let config = RustlsConfig::from_pem_file(
            PathBuf::from("dev").join("cert.pem"),
            PathBuf::from("dev").join("key.pem"),
        )
        .await
        .unwrap();

        let port = self.settings.clone().server.api_port;
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        axum_server::bind_rustls(addr, config)
            .serve(routes_all.into_make_service())
            .await
            .unwrap();
    }

    async fn init_logging(&self) {
        let directory = self.settings.logger.directory.clone();

        let file_appender = tracing_appender::rolling::never(directory, "guardrail.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        let max_level = self.settings.logger.level.parse().unwrap_or(Level::DEBUG);

        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env()
            .unwrap()
            .add_directive("server=debug".parse().unwrap())
            .add_directive("leptos=debug".parse().unwrap())
            .add_directive("app=debug".parse().unwrap());

        let subscriber = FmtSubscriber::builder()
            .with_max_level(max_level)
            .with_ansi(std::io::stdout().is_terminal())
            .with_env_filter(filter)
            .finish()
            .with(fmt::Layer::new().with_writer(non_blocking));

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        tracing_log::LogTracer::init().expect("Failed to set logger");
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

        let token = &self.settings.auth.initial_admin_token;
        let token_hash = hash_token(token).map_err(|_| "Failed to hash token")?;

        let new_token = NewApiToken {
            description: "Default API token".to_string(),
            token_hash,
            product_id: None,
            user_id: None,
            entitlements: vec!["token".to_string()],
            expires_at: None,
        };

        let _token_id = ApiTokenRepo::create(&mut *conn, new_token).await?;
        info!("Created default API token: {}", token);

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let app = GuardrailApp::new();
    app.await.run().await;
}
