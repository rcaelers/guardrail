use apalis::layers::retry::HasherRng;
use apalis::layers::retry::backoff::MakeBackoff;
use apalis::layers::retry::{RetryPolicy, backoff::ExponentialBackoffMaker};
use apalis::prelude::*;
use apalis_cron::CronStream;
use apalis_cron::Tick;
use apalis_redis::{RedisConfig, RedisStorage};
use clap::Parser;
use cron::Schedule;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    Api, Client,
    api::{ObjectMeta, PostParams},
};
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{str::FromStr, sync::Arc, time::Duration};
use tracing::{debug, error, info};

use common::jobs::queue;
use common::token::generate_api_token;
use common::{init_logging, settings::Settings};
use curator::{
    import_crash::ImportCrashProcessor, import_symbol::ImportSymbolProcessor, jobs::ImportCrashJob,
    maintenance, product_listener, product_sync, state::AppState,
};
use repos::Repo;

const SECRET_NAME: &str = "guardrail-initial-admin-token";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

/// Combined state for maintenance tasks that need both AppState and RedisStorage
#[derive(Clone)]
struct MaintenanceState {
    app_state: AppState,
    redis: RedisStorage<ImportCrashJob>,
}

async fn handle_maintenance_tick(
    _tick: Tick,
    state: Data<MaintenanceState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    maintenance::MaintenanceJob::run_all_maintenance_tasks(&state.app_state, &state.redis).await?;
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

    async fn run_apalis(&self, state: AppState, guardrail_db: PgPool) {
        let conn = common::retry_startup("Valkey", || async {
            apalis_redis::connect(self.settings.valkey.uri.clone()).await
        })
        .await;

        let mut redis_manager = common::retry_startup("Valkey connection manager", || async {
            let redis_client = redis::Client::open(self.settings.valkey.uri.as_str())?;
            redis::aio::ConnectionManager::new(redis_client).await
        })
        .await;

        let redis_import_crash = RedisStorage::<ImportCrashJob>::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_CRASH_JOBS),
        );

        if let Err(e) =
            maintenance::MaintenanceJob::run_all_maintenance_tasks(&state, &redis_import_crash)
                .await
        {
            error!("Failed to run startup maintenance tasks: {}", e);
        }

        if let Err(e) = product_sync::sync_products_to_valkey(&state.repo, &mut redis_manager).await
        {
            error!("Failed to run startup product sync: {}", e);
        }

        // Spawn the LISTEN/NOTIFY listener for real-time product cache updates
        let listener_pool = guardrail_db.clone();
        let listener_redis = redis_manager.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = product_listener::listen_for_product_changes(
                    listener_pool.clone(),
                    listener_redis.clone(),
                )
                .await
                {
                    error!("Product change listener failed: {}, restarting...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });

        info!("Start monitoring for import and maintenance jobs");
        let maintenance_schedule =
            Schedule::from_str("0 0 2 * * * *").expect("Invalid cron schedule for maintenance");

        // Clone for import crash worker
        let redis_import_crash_worker = redis_import_crash.clone();
        let state_import_crash = state.clone();

        // Set up import symbol worker
        let redis_import_symbol = RedisStorage::new_with_config(
            conn.clone(),
            RedisConfig::new(queue::IMPORT_SYMBOL_JOBS),
        );
        let state_import_symbol = state.clone();

        // Clone for maintenance worker
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
        let guardrail_db = common::retry_startup("PostgreSQL", || async {
            self.init_guardrail_db().await
        })
        .await;
        let repo = Repo::new(guardrail_db.clone());

        common::retry_startup("curator bootstrap", || async {
            self.ensure_default_api_token(&repo).await
        })
        .await;

        let store = common::init_s3_object_store(self.settings.clone()).await;

        let state = AppState::new(repo, self.settings.clone(), store);

        info!("Maintenance worker is starting");
        self.run_apalis(state.clone(), guardrail_db).await;
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

        sqlx::migrate!("../../../migrations").run(&pool).await?;

        Ok(pool)
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

        if let Err(err) = self.create_k8s_initial_token_secret(&token).await {
            tracing::warn!("Failed to create initial token secret: {}", err);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let app = MaintenanceWorker::new(&args.config_dir).await;
    app.run().await;
}
