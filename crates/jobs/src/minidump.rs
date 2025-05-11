use std::sync::Arc;

use apalis::prelude::{Context, Data, Worker};
use bytes::Bytes;
use data::{crash::NewCrash, product::Product};
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

        MinidumpProcessor { storage, repo }
    }

    async fn handle_job(
        &self,
        crash_id: uuid::Uuid,
        crash_info: serde_json::Value,
    ) -> Result<(), JobError> {
        let mut tx = self.repo.begin_admin().await?;

        let minidump_path = crash_info["minidump"]["storage_path"]
            .as_str()
            .ok_or_else(|| {
                error!("No minidump found for crash {}", crash_info["id"]);
                JobError::Failure("no minidump found".to_string())
            })?;
        let data = self.get_minidump_object(minidump_path).await?;

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
        let _json: Value = serde_json::from_slice(&json_output).map_err(|e| {
            error!("Failed to parse minidump json: {:?}", e);
            JobError::Failure("failed to parse minidump json".to_string())
        })?;

        let product_id = crash_info["product_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("product_id is missing".to_string()))?
            .parse::<uuid::Uuid>()
            .map_err(|_| JobError::Failure("invalid product_id format".to_string()))?;

        let crash = NewCrash {
            id: Some(crash_id),
            // report: Some(json),
            // state: State::Complete,
            ..Default::default()
        };

        let product = ProductRepo::get_by_id(&mut *tx, product_id)
            .await
            .map_err(|_| JobError::Failure(format!("failed to get product {product_id}")))?
            .ok_or_else(|| JobError::Failure(format!("no such product {product_id}")))?;

        Self::create_crash(&mut *tx, crash.clone(), product).await?;
        info!("Updated crash report with ID: {:?}", crash.id);
        Ok(())
    }

    // async fn retrieve_data<E>(
    //     &self,
    //     tx: &mut E,
    //     job: MinidumpJob,
    // ) -> Result<(Crash, Product), JobError>
    // where
    //     for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    // {
    //     let crash_info = job.crash;
    //     let product_id = crash_info["product_id"]
    //         .as_str()
    //         .ok_or_else(|| {
    //             error!("Product ID is missing in crash info");
    //             JobError::Failure("product_id is missing".to_string())
    //         })?
    //         .parse::<uuid::Uuid>()
    //         .map_err(|_| {
    //             error!("Invalid product ID format in crash info");
    //             JobError::Failure("invalid product_id format".to_string())
    //         })?;

    //     let product = ProductRepo::get_by_id(&mut *tx, product_id)
    //         .await
    //         .map_err(|_| {
    //             error!("Failed to get product {}", product_id);
    //             JobError::Failure(format!("failed to get product {}", product_id))
    //         })?
    //         .ok_or_else(|| {
    //             error!("No such product  {}", product_id);
    //             JobError::Failure(format!("no such productt {}", product_id))
    //         })?;

    //     Ok((crash_info, product))
    // }

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

    async fn create_crash<E>(
        tx: &mut E,
        crash: NewCrash,
        product: Product,
    ) -> Result<uuid::Uuid, JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let id = CrashRepo::create(&mut *tx, crash).await.map_err(|e| {
            error!("Failed to store crash report for {} ({:?})", product.name, e);
            JobError::Failure("failed to store crash report".to_string())
        })?;
        Ok(id)
    }

    pub async fn process(
        job: MinidumpJob,
        _worker: Worker<Context>,
        state: Data<AppState>,
    ) -> Result<(), JobError> {
        let crash_id = job.crash["crash_id"]
            .as_str()
            .ok_or_else(|| {
                error!("Crash ID is missing in job");
                JobError::Failure("crash_id is missing".to_string())
            })?
            .parse::<uuid::Uuid>()
            .map_err(|_| {
                error!("Invalid crash ID format in job");
                JobError::Failure("invalid crash_id format".to_string())
            })?;
        info!("Process minidump: {}", crash_id);

        let processor = MinidumpProcessor::new(state);
        processor.handle_job(crash_id, job.crash).await?;
        info!("Successfully processed minidump for crash ID: {}", crash_id);
        Ok(())
    }
}
