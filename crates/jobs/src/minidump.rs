use std::sync::Arc;

use apalis::prelude::{Context, Data, Worker};
use bytes::Bytes;
use data::{
    crash::{Crash, State},
    product::Product,
};
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::Symbolizer;
use object_store::{ObjectStore, path::Path};
use repos::{Repo, crash::CrashRepo, product::ProductRepo};
use serde_json::Value;
use sqlx::Postgres;
use tracing::{error, info};

use crate::{
    error::JobError, jobs::MinidumpJob, state::AppState, symbol_provider::s3_symbol_supplier,
};

pub struct MinidumpProcessor {
    storage: Arc<dyn ObjectStore>,
    repo: Repo,
}

impl MinidumpProcessor {
    pub fn new(s: Data<AppState>) -> MinidumpProcessor {
        let storage = s.storage.clone();
        let repo = s.repo.clone();

        MinidumpProcessor {
            storage,
            repo,
        }
    }

    async fn handle_job(&self, job: MinidumpJob) -> Result<(), JobError> {
        let mut tx = self.repo.begin_admin().await.map_err(|e| {
            error!("Failed to start transaction: {:?}", e);
            JobError::Failure("failed to start transaction".to_string())
        })?;

        let (mut crash, product) = self.retrieve_data(&mut *tx, job).await?;

        let minidump = crash.minidump.ok_or_else(|| {
            error!("No minidump found for crash {}", crash.id);
            JobError::Failure("no minidump found".to_string())
        })?;
        let path = format!("minidumps/{}", minidump);
        let data = self.get_minidump_object(&path).await?;

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let dump = Minidump::read(data).map_err(|e| {
            error!("Failed to read minidump: {:?}", e);
            JobError::Failure("failed to read minidump".to_string())
        })?;
        let provider = Symbolizer::new(s3_symbol_supplier(self.storage.clone(), self.repo.clone()));
        let state = minidump_processor::process_minidump_with_options(&dump, &provider, options)
            .await
            .expect("Failed to process minidump");

        let mut json_output = Vec::new();
        state
            .print_json(&mut json_output, false)
            .expect("Failed to print json");
        let json: Value = serde_json::from_slice(&json_output).map_err(|e| {
            error!("Failed to parse minidump json: {:?}", e);
            JobError::Failure("failed to parse minidump json".to_string())
        })?;

        crash.report = Some(json);
        crash.state = State::Complete;
        Self::update_crash(&mut *tx, crash.clone(), product).await?;
        info!("Updated crash report with ID: {:?}", crash.id);
        Ok(())
    }

    async fn retrieve_data<E>(
        &self,
        tx: &mut E,
        job: MinidumpJob,
    ) -> Result<(Crash, Product), JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
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
            })?
            .ok_or_else(|| {
                error!("No such product  {}", crash.product_id);
                JobError::Failure(format!("no such productt {}", crash.product_id))
            })?;

        Ok((crash, product))
    }

    async fn get_minidump_object(&self, path: &str) -> Result<Bytes, JobError> {
        let object = self.storage.get(&Path::from(path)).await.map_err(|err| {
            error!("Failed to get minidump object: {err}");
            JobError::Failure("Failed to retrieve minidump".to_string())
        })?;
        info!("Got minidump object: {:?}", object);
        let data = object.bytes().await.map_err(|err| {
            error!("Failed to read minidump object: {err}");
            JobError::Failure("Failed to retrieve minidump ".to_string())
        })?;
        Ok(data)
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
        _worker: Worker<Context>,
        state: Data<AppState>,
    ) -> Result<(), JobError> {
        info!("Process minidump: {}", job.crash_id);

        let processor = MinidumpProcessor::new(state);
        processor.handle_job(job.clone()).await?;
        info!("Successfully processed minidump for crash ID: {}", job.crash_id);
        Ok(())
    }
}
