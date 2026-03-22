use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_redis::{RedisConfig, RedisStorage};
use clap::Parser;
use std::{sync::Arc, time::Duration};
use tracing::{debug, info};

use common::jobs::queue;
use common::{init_logging, settings::Settings};
use processor::{
    jobs::{ImportCrashJob, ImportSymbolJob, MinidumpJob, SymbolJob},
    minidump::MinidumpProcessor,
    state::AppState,
    symbols::SymbolProcessor,
};
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
        let conn = apalis_redis::connect(self.settings.job_server.redis_uri.clone())
            .await
            .expect("Failed to connect to Redis/Valkey");

        let redis_minidump = RedisStorage::<MinidumpJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::MINIDUMP_JOBS),
        );

        let redis_symbol = RedisStorage::<SymbolJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::SYMBOL_JOBS),
        );

        // Create storages for enqueueing jobs to the curator
        let redis_import_crash = RedisStorage::<ImportCrashJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_CRASH_JOBS),
        );

        let redis_import_symbol = RedisStorage::<ImportSymbolJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_SYMBOL_JOBS),
        );

        info!("Start monitoring for minidumps and symbols");

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
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let app = MinidumpWorker::new(&args.config_dir).await;
    app.run().await;
}
