use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_postgres::{Config, PostgresStorage};
use clap::Parser;
use common::jobs::queue;
use common::{init_logging, settings::Settings};
use processor::{
    jobs::{ImportCrashJob, ImportSymbolJob, MinidumpJob, SymbolJob},
    minidump::MinidumpProcessor,
    state::AppState,
    symbols::SymbolProcessor,
};
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{sync::Arc, time::Duration};
use tracing::{debug, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

struct MinidumpWorker {
    settings: Arc<Settings>,
}

impl MinidumpWorker {
    async fn new(config_dir: &str) -> Self {
        Self {
            settings: Arc::new(
                Settings::with_config_dir(config_dir).expect("Failed to load settings"),
            ),
        }
    }

    async fn run_apalis(&self, state: AppState) {
        let worker_db = self.init_worker_db().await.unwrap();

        PostgresStorage::setup(&worker_db)
            .await
            .expect("unable to run migrations for postgres");

        let config = Config::new(queue::MINIDUMP_JOBS);
        let pg_minidump = PostgresStorage::<MinidumpJob>::new_with_notify(&worker_db, &config);

        let symbol_config = Config::new(queue::SYMBOL_JOBS);
        let pg_symbol = PostgresStorage::<SymbolJob>::new_with_notify(&worker_db, &symbol_config);

        // Create storages for enqueueing jobs to the curator
        let import_crash_config = Config::new(queue::IMPORT_CRASH_JOBS);
        let pg_import_crash =
            PostgresStorage::<ImportCrashJob>::new_with_notify(&worker_db, &import_crash_config);

        let import_symbol_config = Config::new(queue::IMPORT_SYMBOL_JOBS);
        let pg_import_symbol =
            PostgresStorage::<ImportSymbolJob>::new_with_notify(&worker_db, &import_symbol_config);

        info!("Start monitoring for minidumps and symbols");

        let state1 = state.clone();
        let pg_minidump1 = pg_minidump.clone();
        let pg_import_crash1 = pg_import_crash.clone();

        let state2 = state.clone();
        let pg_symbol1 = pg_symbol.clone();
        let pg_import_symbol1 = pg_import_symbol.clone();

        Monitor::new()
            .register(move |_idx| {
                let backoff = ExponentialBackoffMaker::new(
                    Duration::from_millis(1000),
                    Duration::from_millis(5000),
                    1.25,
                    HasherRng::default(),
                )
                .expect("Failed to create backoff")
                .make_backoff();

                WorkerBuilder::new("minidump-processor")
                    .backend(pg_minidump1.clone())
                    .data(state1.clone())
                    .data(pg_import_crash1.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(MinidumpProcessor::process)
            })
            .register(move |_idx| {
                let backoff = ExponentialBackoffMaker::new(
                    Duration::from_millis(1000),
                    Duration::from_millis(5000),
                    1.25,
                    HasherRng::default(),
                )
                .expect("Failed to create backoff")
                .make_backoff();

                WorkerBuilder::new("symbol-processor")
                    .backend(pg_symbol1.clone())
                    .data(state2.clone())
                    .data(pg_import_symbol1.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(SymbolProcessor::process)
            })
            .on_event(|_worker, e| debug!("Apalis event: {e}"))
            .run_with_signal(async {
                tokio::signal::ctrl_c().await?;
                info!("Shutting down the system");
                Ok(())
            })
            .await
            .expect("Failed to run the monitor");
        info!("Minidump processor ends");
    }

    async fn run(&self) {
        init_logging().await;

        info!("Starting minidump processor");
        let store = common::init_s3_object_store(self.settings.clone()).await;

        let state = AppState::new(self.settings.clone(), store);

        info!("Minidump processor is starting");
        self.run_apalis(state.clone()).await;
        info!("Minidump processor has stopped");
    }

    async fn init_worker_db(&self) -> Result<PgPool, sqlx::Error> {
        let database_url = &self.settings.job_server.db_uri;
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
    let args = CliArgs::parse();
    let app = MinidumpWorker::new(&args.config_dir).await;
    app.run().await;
}
