use apalis::layers::retry::RetryPolicy;
use apalis::prelude::*;
use apalis_sql::{
    Config,
    postgres::{PgListen, PostgresStorage},
};
use axum::{Router, extract::State, http::StatusCode, routing::get};
use clap::Parser;
use common::{init_logging, settings::Settings};
use jobs::{minidump::MinidumpProcessor, state::AppState};
use repos::Repo;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

struct GuardrailJobs {
    settings: Arc<Settings>,
}

impl GuardrailJobs {
    async fn new(config_dir: &str) -> Self {
        Self {
            settings: Arc::new(
                Settings::with_config_dir(config_dir).expect("Failed to load settings"),
            ),
        }
    }

    async fn live() -> StatusCode {
        StatusCode::OK
    }

    async fn ready(State(state): State<AppState>) -> StatusCode {
        let mut conn = match state.repo.acquire_admin().await {
            Ok(conn) => conn,
            Err(err) => {
                error!("Health check failed to get database connection: {}", err);
                return StatusCode::SERVICE_UNAVAILABLE;
            }
        };

        if sqlx::query("SELECT 1").execute(&mut *conn).await.is_ok() {
            return StatusCode::OK;
        }
        StatusCode::SERVICE_UNAVAILABLE
    }

    async fn run_http(&self, state: AppState) -> impl std::future::Future<Output = ()> {
        let routes_all = Router::new()
            .route("/live", get(Self::live))
            .route("/ready", get(Self::ready))
            .layer(TimeoutLayer::new(Duration::from_secs(60)))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        async {
            let addr = SocketAddr::from(([0, 0, 0, 0], self.settings.job_server.port));
            axum_server::bind(addr)
                .serve(routes_all.into_make_service())
                .await
                .unwrap()
        }
    }

    async fn run_apalis(&self, state: AppState) -> impl std::future::Future<Output = ()> {
        let worker_db = self.init_worker_db().await.unwrap();

        PostgresStorage::setup(&worker_db)
            .await
            .expect("unable to run migrations for postgres");

        let mut pg = PostgresStorage::new_with_config(
            worker_db.clone(),
            Config::new("guardrail::Jobs").set_poll_interval(std::time::Duration::from_secs(5)),
        );
        let mut listener = PgListen::new(worker_db)
            .await
            .expect("Failed to create listener");

        listener.subscribe_with(&mut pg);

        tokio::spawn(async move {
            listener.listen().await.unwrap();
        });

        info!("Start monitoring for minidumps");
        let state = state.clone();
        async move {
            Monitor::new()
                .register({
                    WorkerBuilder::new("minidump")
                        .data(state.clone())
                        .retry(RetryPolicy::retries(5))
                        .enable_tracing()
                        .backend(pg)
                        .build_fn(MinidumpProcessor::process)
                })
                .on_event(|e| debug!("Apalis event: {e}"))
                .run_with_signal(async {
                    tokio::signal::ctrl_c().await?;
                    info!("Shutting down the system");
                    Ok(())
                })
                .await
                .expect("Failed to run the monitor")
        }
    }

    async fn run(&self) {
        init_logging().await;

        info!("Starting jobs server");
        let guardrail_db = self.init_guardrail_db().await.unwrap();
        let repo = Repo::new(guardrail_db.clone());
        let store = common::init_s3_object_store(self.settings.clone()).await;

        let state = AppState {
            repo,
            settings: self.settings.clone(),
            storage: store,
        };

        let http = self.run_http(state.clone());
        let apalis = self.run_apalis(state.clone());
        let _res = tokio::join!(http, apalis);
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
    let app = GuardrailJobs::new(&args.config_dir).await;
    app.run().await;
}
