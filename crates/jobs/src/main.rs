use apalis::layers::retry::RetryPolicy;
use apalis::prelude::*;
use apalis_sql::{
    Config,
    postgres::{PgListen, PostgresStorage},
};
use common::settings::Settings;
use jobs::{minidump::MinidumpProcessor, state::AppState};
use repos::Repo;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::sync::Arc;
use std::io::IsTerminal;
use tracing::{Level, debug, info};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, FmtSubscriber};
use tracing_subscriber::{EnvFilter, fmt};

struct GuardrailJobs {
    settings: Arc<Settings>,
}

impl GuardrailJobs {
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
        let repo = Repo::new(db.clone());
        let store = Arc::new(
            object_store::aws::AmazonS3Builder::from_env()
                .with_url(settings.clone().server.store.clone())
                .build()
                .expect("Failed to create object store"),
        );

        let state = AppState {
            repo,
            settings: settings.clone(),
            storage: store,
        };

        PostgresStorage::setup(&db)
            .await
            .expect("unable to run migrations for postgres");

        let mut pg = PostgresStorage::new_with_config(db.clone(), Config::new("guardrail::Jobs"));
        let mut listener = PgListen::new(db).await.expect("Failed to create listener");

        listener.subscribe_with(&mut pg);

        tokio::spawn(async move {
            listener.listen().await.unwrap();
        });

        Monitor::new()
            .register({
                WorkerBuilder::new("minidump")
                    .data(state.clone())
                    .retry(RetryPolicy::retries(5))
                    .enable_tracing()
                    .backend(pg)
                    .build_fn(MinidumpProcessor::process)
            })
            .on_event(|e| debug!("{e}"))
            .run_with_signal(async {
                tokio::signal::ctrl_c().await?;
                info!("Shutting down the system");
                Ok(())
            })
            .await
            .expect("Failed to run the monitor");
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
}

#[tokio::main]
async fn main() {
    let app = GuardrailJobs::new();
    app.await.run().await;
}
