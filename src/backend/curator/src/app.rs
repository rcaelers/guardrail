use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_redis::{ConnectionManager, RedisConfig, RedisStorage};
use surrealdb::opt::auth::Root;
use tracing::{debug, error, info};

use common::jobs::queue;
use common::settings::Settings;
use repos::Repo;

use crate::import_crash::ImportCrashProcessor;
use crate::import_symbol::ImportSymbolProcessor;
use crate::jobs::ImportCrashJob;
use crate::maintenance;
use crate::product_listener;
use crate::product_sync;
use crate::state::AppState;

#[derive(Clone)]
struct MaintenanceState {
    app_state: AppState,
    redis: RedisStorage<ImportCrashJob>,
}

async fn handle_maintenance_tick(
    _tick: apalis_cron::Tick,
    state: Data<MaintenanceState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    maintenance::MaintenanceJob::run_all_maintenance_tasks(&state.app_state, &state.redis).await?;
    Ok(())
}

pub struct GuardrailCuratorApp {
    state: AppState,
    conn: ConnectionManager,
    redis_manager: redis::aio::ConnectionManager,
}

impl GuardrailCuratorApp {
    pub fn new(
        state: AppState,
        conn: ConnectionManager,
        redis_manager: redis::aio::ConnectionManager,
    ) -> Self {
        Self {
            state,
            conn,
            redis_manager,
        }
    }

    /// Bootstrap from settings: connect to SurrealDB, Valkey, and S3, then build internal state.
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

        info!("Connected to SurrealDB at {}", settings.database.endpoint);

        let conn = apalis_redis::connect(settings.valkey.uri.clone())
            .await
            .expect("Failed to connect to Valkey (apalis)");

        let redis_client = redis::Client::open(settings.valkey.uri.as_str())
            .expect("Failed to create Redis client");
        let redis_manager = redis::aio::ConnectionManager::new(redis_client)
            .await
            .expect("Failed to create Redis connection manager");

        let store = common::init_s3_object_store(settings.clone()).await;
        let repo = Repo::new(db);
        let state = AppState::new(repo, settings, store);

        Self {
            state,
            conn,
            redis_manager,
        }
    }

    pub async fn run(&self, shutdown: impl Future<Output = std::io::Result<()>> + Send) {
        self.sync_products().await;
        self.spawn_product_listener();
        self.run_workers(shutdown).await;
    }

    async fn sync_products(&self) {
        let mut redis_manager = self.redis_manager.clone();
        if let Err(e) =
            product_sync::sync_products_to_valkey(&self.state.repo, &mut redis_manager).await
        {
            error!("Failed to run startup product sync: {}", e);
        }
    }

    fn spawn_product_listener(&self) {
        let listener_db = self.state.repo.db.clone();
        let listener_redis = self.redis_manager.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = product_listener::listen_for_product_changes(
                    listener_db.clone(),
                    listener_redis.clone(),
                )
                .await
                {
                    error!("Product change listener failed: {}, restarting...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    async fn run_workers(&self, shutdown: impl Future<Output = std::io::Result<()>> + Send) {
        let state = self.state.clone();
        let conn = self.conn.clone();

        let redis_import_crash = RedisStorage::<ImportCrashJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_CRASH_JOBS),
        );
        let redis_import_symbol = RedisStorage::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_SYMBOL_JOBS),
        );

        if let Err(e) =
            maintenance::MaintenanceJob::run_all_maintenance_tasks(&state, &redis_import_crash)
                .await
        {
            error!("Failed to run startup maintenance tasks: {}", e);
        }

        let maintenance_schedule = cron::Schedule::from_str("0 0 2 * * * *")
            .expect("Invalid cron schedule for maintenance");

        let state_import_crash = state.clone();
        let state_import_symbol = state.clone();
        let redis_import_crash_worker = redis_import_crash.clone();
        let maintenance_state = MaintenanceState {
            app_state: state.clone(),
            redis: redis_import_crash.clone(),
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
                    .backend(redis_import_crash_worker.clone())
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
                    .backend(redis_import_symbol.clone())
                    .data(state_import_symbol.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(ImportSymbolProcessor::process)
            })
            .register(move |_idx| {
                WorkerBuilder::new("maintenance")
                    .backend(apalis_cron::CronStream::new(maintenance_schedule.clone()))
                    .data(maintenance_state.clone())
                    .enable_tracing()
                    .build(handle_maintenance_tick)
            })
            .on_event(|_worker, e| debug!("Apalis event: {e}"))
            .run_with_signal(shutdown)
            .await
            .expect("Failed to run the monitor");
    }
}
