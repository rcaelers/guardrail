use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_redis::{ConnectionManager, RedisConfig, RedisStorage};
use tracing::debug;

use common::jobs::queue;
use common::retry_startup;
use common::settings::Settings;

use crate::jobs::{ImportCrashJob, ImportSymbolJob, MinidumpJob, SymbolJob};
use crate::minidump::MinidumpProcessor;
use crate::state::AppState;
use crate::symbols::SymbolProcessor;

pub struct GuardrailProcessorApp {
    state: AppState,
    conn: ConnectionManager,
    redis_manager: redis::aio::ConnectionManager,
}

impl GuardrailProcessorApp {
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

    /// Bootstrap from settings: connect to Valkey and S3, build internal state.
    pub async fn from_settings(settings: Arc<Settings>) -> Self {
        let conn = retry_startup("Valkey (apalis)", || {
            let uri = settings.valkey.uri.clone();
            async move { apalis_redis::connect(uri).await }
        })
        .await;

        let redis_client = redis::Client::open(settings.valkey.uri.as_str())
            .expect("Failed to create Redis client");
        let redis_manager = retry_startup("Valkey (redis)", || {
            let redis_client = redis_client.clone();
            async move { redis::aio::ConnectionManager::new(redis_client).await }
        })
        .await;

        let store = common::init_s3_object_store(settings.clone()).await;
        let state = AppState::new(settings, store);

        Self {
            state,
            conn,
            redis_manager,
        }
    }

    pub async fn run(&self, shutdown: impl Future<Output = std::io::Result<()>> + Send) {
        let redis_health = self.redis_manager.clone();
        common::spawn_health_server(9090, move || {
            let mut conn = redis_health.clone();
            Box::pin(async move {
                redis::cmd("PING")
                    .query_async::<String>(&mut conn)
                    .await
                    .is_ok()
            })
        });
        self.run_workers(shutdown).await;
    }

    async fn run_workers(&self, shutdown: impl Future<Output = std::io::Result<()>> + Send) {
        let state = self.state.clone();
        let conn = self.conn.clone();

        let redis_minidump = RedisStorage::<MinidumpJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::MINIDUMP_JOBS),
        );
        let redis_symbol = RedisStorage::<SymbolJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::SYMBOL_JOBS),
        );
        let redis_import_crash = RedisStorage::<ImportCrashJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_CRASH_JOBS),
        );
        let redis_import_symbol = RedisStorage::<ImportSymbolJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_SYMBOL_JOBS),
        );

        let state1 = state.clone();
        let redis_minidump1 = redis_minidump.clone();
        let redis_import_crash1 = redis_import_crash.clone();

        let state2 = state.clone();
        let redis_symbol1 = redis_symbol.clone();
        let redis_import_symbol1 = redis_import_symbol.clone();

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
                    .backend(redis_minidump1.clone())
                    .data(state1.clone())
                    .data(redis_import_crash1.clone())
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
                    .backend(redis_symbol1.clone())
                    .data(state2.clone())
                    .data(redis_import_symbol1.clone())
                    .retry(RetryPolicy::retries(5).with_backoff(backoff))
                    .enable_tracing()
                    .concurrency(2)
                    .build(SymbolProcessor::process)
            })
            .on_event(|_worker, e| debug!("Apalis event: {e}"))
            .run_with_signal(shutdown)
            .await
            .expect("Failed to run the monitor");
    }
}
