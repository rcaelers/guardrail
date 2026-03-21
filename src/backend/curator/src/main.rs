use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_cron::CronStream;
use apalis_cron::Tick;
use apalis_postgres::{Config, PostgresStorage};
use clap::Parser;
use cron::Schedule;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{str::FromStr, sync::Arc, time::Duration};
use tracing::{debug, error, info};

use common::{init_logging, settings::Settings};
use common::jobs::queue;
use curator::{
    import_crash::ImportCrashProcessor,
    import_symbol::ImportSymbolProcessor,
    jobs::ImportCrashJob,
    maintenance::{self, NotifyPostgresStorage},
    state::AppState,
};
use repos::Repo;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

/// Combined state for maintenance tasks that need both AppState and PostgresStorage
#[derive(Clone)]
struct MaintenanceState {
    app_state: AppState,
    pg: NotifyPostgresStorage<ImportCrashJob>,
}

async fn handle_maintenance_tick(
    _tick: Tick,
    state: Data<MaintenanceState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    maintenance::MaintenanceJob::run_all_maintenance_tasks(&state.app_state, &state.pg).await?;
    Ok(())
}

struct MaintenanceWorker {
    settings: Arc<Settings>,
}

impl MaintenanceWorker {
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

        let config = Config::new(queue::IMPORT_CRASH_JOBS);
        let pg = PostgresStorage::new_with_notify(&worker_db, &config);

        if let Err(e) = maintenance::MaintenanceJob::run_all_maintenance_tasks(&state, &pg).await {
            error!("Failed to run startup maintenance tasks: {}", e);
        }

        info!("Start monitoring for import and maintenance jobs");
        let maintenance_schedule =
            Schedule::from_str("0 0 2 * * * *").expect("Invalid cron schedule for maintenance");

        // Clone for import crash worker
        let pg_import_crash = pg.clone();
        let state_import_crash = state.clone();

        // Set up import symbol worker
        let import_symbol_config = Config::new(queue::IMPORT_SYMBOL_JOBS);
        let pg_import_symbol = PostgresStorage::new_with_notify(&worker_db, &import_symbol_config);
        let state_import_symbol = state.clone();

        // Clone for maintenance worker
        let maintenance_state = MaintenanceState {
            app_state: state.clone(),
            pg: pg.clone(),
        };

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

                WorkerBuilder::new("import-crash")
                    .backend(pg_import_crash.clone())
                    .data(state_import_crash.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(ImportCrashProcessor::process)
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

                WorkerBuilder::new("import-symbol")
                    .backend(pg_import_symbol.clone())
                    .data(state_import_symbol.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(ImportSymbolProcessor::process)
            })
            .register(move |_idx| {
                WorkerBuilder::new("maintenance")
                    .backend(CronStream::new(maintenance_schedule.clone()))
                    .data(maintenance_state.clone())
                    .enable_tracing()
                    .build(handle_maintenance_tick)
            })
            .on_event(|_worker, e| debug!("Apalis event: {e}"))
            .run_with_signal(async {
                tokio::signal::ctrl_c().await?;
                info!("Shutting down the system");
                Ok(())
            })
            .await
            .expect("Failed to run the monitor");
        info!("Maintenance worker ends");
    }

    async fn run(&self) {
        init_logging().await;

        info!("Starting maintenance worker");
        let guardrail_db = self.init_guardrail_db().await.unwrap();
        let repo = Repo::new(guardrail_db.clone());
        let store = common::init_s3_object_store(self.settings.clone()).await;

        let state = AppState::new(repo, self.settings.clone(), store);

        info!("Maintenance worker is starting");
        self.run_apalis(state.clone()).await;
        info!("Maintenance worker has stopped");
    }

    async fn init_guardrail_db(&self) -> Result<PgPool, sqlx::Error> {
        let database_url = &self.settings.database.db_uri;
        let mut opts: PgConnectOptions = database_url.parse()?;
        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Ok(pool)
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
    let app = MaintenanceWorker::new(&args.config_dir).await;
    app.run().await;
}
