use apalis::layers::retry::RetryPolicy;
use apalis::prelude::*;
use apalis_sql::{
    Config,
    postgres::{PgListen, PostgresStorage},
};
use common::{init_logging, settings::Settings};
use jobs::{minidump::MinidumpProcessor, state::AppState};
use repos::Repo;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::sync::Arc;
use tracing::{debug, info};

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
        init_logging().await;

        let settings = Arc::new(Settings::new().expect("Failed to load settings"));

        let db = self.init_db().await.unwrap();
        let repo = Repo::new(db.clone());
        let store = common::init_s3_object_store(self.settings.clone()).await;

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
