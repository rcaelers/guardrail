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
use common::settings::Settings;

use crate::jobs::{ImportCrashJob, ImportSymbolJob, MinidumpJob, SymbolJob};
use crate::minidump::MinidumpProcessor;
use crate::state::AppState;
use crate::symbols::SymbolProcessor;

pub struct GuardrailProcessorApp {
    state: AppState,
    conn: ConnectionManager,
}

impl GuardrailProcessorApp {
    pub fn new(state: AppState, conn: ConnectionManager) -> Self {
        Self { state, conn }
    }

    /// Bootstrap from settings: connect to Valkey and S3, build internal state.
    pub async fn from_settings(settings: Arc<Settings>) -> Self {
        let conn = apalis_redis::connect(settings.valkey.uri.clone())
            .await
            .expect("Failed to connect to Valkey (apalis)");

        let store = common::init_s3_object_store(settings.clone()).await;
        let state = AppState::new(settings, store);

        Self { state, conn }
    }

    pub async fn run(&self, shutdown: impl Future<Output = std::io::Result<()>> + Send) {
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
