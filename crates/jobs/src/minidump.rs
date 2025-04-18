use std::{path::PathBuf, sync::Arc};

use apalis::prelude::{Context, Data, Worker};
use common::settings::Settings;
use data::{crash::Crash, product::Product};
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::{Symbolizer, simple_symbol_supplier};
use repos::{crash::CrashRepo, product::ProductRepo, version::VersionRepo};
use serde_json::Value;
use sqlx::Postgres;
use tracing::{debug, error, info};

use crate::{error::JobError, jobs::MinidumpJob, state::AppState};

pub struct MinidumpProcessor {
    settings: Arc<Settings>,
}

impl MinidumpProcessor {
    async fn process_minidump_file(
        &self,
        minidump_file: PathBuf,
    ) -> Result<serde_json::Value, JobError> {
        let dump = Minidump::read_path(minidump_file)?;

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let path = std::path::Path::new(&self.settings.server.base_path)
            .join("symbols")
            .to_path_buf();
        debug!("provider: {:?}", path);
        let provider = Symbolizer::new(simple_symbol_supplier(vec![path]));

        let state =
            minidump_processor::process_minidump_with_options(&dump, &provider, options).await?;

        let mut json_output = Vec::new();
        state.print_json(&mut json_output, false).map_err(|e| {
            error!("Failed to print minidump json: {:?}", e);
            JobError::Failure("failed to print minidump json".to_string())
        })?;
        let json: Value = serde_json::from_slice(&json_output).map_err(|e| {
            error!("Failed to parse minidump json: {:?}", e);
            JobError::Failure("failed to parse minidump json".to_string())
        })?;

        debug!("json: {:?}", json);
        Ok(json)
    }

    async fn update_crash<E>(
        tx: &mut E,
        crash: Crash,
        product: Product,
    ) -> Result<uuid::Uuid, JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let id = CrashRepo::update(&mut *tx, crash)
            .await
            .map_err(|e| {
                error!("Failed to store crash report for {} ({:?})", product.name, e);
                JobError::Failure("failed to store crash report".to_string())
            })?
            .ok_or_else(|| {
                error!("Failed to store crash report for {}", product.name);
                JobError::Failure("failed to store crash report".to_string())
            })?;
        Ok(id)
    }

    pub async fn process(
        job: MinidumpJob,
        worker: Worker<Context>,
        state: Data<AppState>,
    ) -> Result<(), JobError> {
        info!("Process minidump: {}", job.crash_id);

        let mut tx = state.repo.begin_admin().await.map_err(|e| {
            error!("Failed to start transaction: {:?}", e);
            JobError::Failure("failed to start transaction".to_string())
        })?;

        let crash = CrashRepo::get_by_id(&mut *tx, job.crash_id)
            .await
            .map_err(|e| {
                error!("Failed to get crash report: {:?}", e);
                JobError::Failure("failed to get crash report".to_string())
            })?
            .ok_or_else(|| {
                error!("No such crash report {}", job.crash_id);
                JobError::Failure(format!("no such crash report {}", job.crash_id))
            })?;

        let product = ProductRepo::get_by_id(&mut *tx, crash.product_id)
            .await
            .map_err(|_| {
                error!("Failed to get product {}", crash.product_id);
                JobError::Failure(format!("failed to get product {}", crash.product_id))
            })?;

        let version = VersionRepo::get_by_id(&mut *tx, crash.version_id)
            .await
            .map_err(|_| {
                error!("Failed to get version {}", crash.version_id);
                JobError::Failure(format!("failed to get version {}", crash.version_id))
            })?;

        // process_minidump_file(minidump_file).await.map_err(|e| {
        //     error!("Failed to process minidump file: {:?}", e);
        //     JobError::Failure("Failed to process minidump file".to_string())
        // })?;

        Ok(())
    }
}
