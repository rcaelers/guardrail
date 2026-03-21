use apalis::prelude::*;
use apalis_postgres::PostgresStorage;
use minidump_unwind::SymbolFile;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    error::JobError,
    jobs::{ImportSymbolJob, SymbolJob},
    state::AppState,
};

pub struct SymbolProcessor {
    storage: Arc<dyn ObjectStore>,
}

impl SymbolProcessor {
    pub fn new(s: Data<AppState>) -> SymbolProcessor {
        SymbolProcessor {
            storage: s.storage.clone(),
        }
    }

    #[instrument(skip(self, symbol_info, pg_storage), fields(storage_path))]
    async fn handle_job(
        &self,
        symbol_upload_id: uuid::Uuid,
        symbol_info: serde_json::Value,
        pg_storage: &PostgresStorage<ImportSymbolJob>,
    ) -> Result<(), JobError> {
        info!("SymbolProcessor handling job: {}", symbol_upload_id);

        let storage_path = symbol_info["storage_path"].as_str().ok_or_else(|| {
            error!("No storage_path found in symbol_info");
            JobError::Failure("storage_path is missing".to_string())
        })?;

        // Read the symbol file from S3
        let data = self.get_symbol_object(storage_path).await?;

        // Validate by parsing as a Breakpad symbol file
        SymbolFile::from_bytes(&data).map_err(|e| {
            error!("Symbol file validation failed: {:?}", e);
            JobError::Failure(format!("symbol file validation failed: {e}"))
        })?;
        info!("Symbol file validated successfully: {}", storage_path);

        // Write processed symbol info to S3
        self.write_processed_symbol(symbol_upload_id, &symbol_info)
            .await?;
        info!("Wrote processed symbol info to S3: {}", symbol_upload_id);

        // Enqueue ImportSymbolJob for the curator to import into database
        let import_job = ImportSymbolJob { symbol_upload_id };
        pg_storage.clone().push(import_job).await.map_err(|e| {
            error!(error = ?e, "Failed to enqueue ImportSymbolJob");
            JobError::ApalisError(format!("failed to enqueue ImportSymbolJob: {:?}", e))
        })?;
        info!("Enqueued ImportSymbolJob for symbol upload: {}", symbol_upload_id);

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_symbol_object(&self, path: &str) -> Result<bytes::Bytes, JobError> {
        let object = self.storage.get(&Path::from(path)).await.map_err(|err| {
            error!("Failed to get symbol object from {}: {err}", path);
            JobError::Failure("Failed to retrieve symbol file".to_string())
        })?;
        let data = object.bytes().await.map_err(|err| {
            error!("Failed to read symbol object: {err}");
            JobError::Failure("Failed to read symbol file".to_string())
        })?;
        Ok(data)
    }

    #[instrument(skip(self, symbol_info), fields(symbol_upload_id = %symbol_upload_id))]
    async fn write_processed_symbol(
        &self,
        symbol_upload_id: uuid::Uuid,
        symbol_info: &serde_json::Value,
    ) -> Result<(), JobError> {
        let json_bytes = serde_json::to_vec_pretty(symbol_info).map_err(|e| {
            error!("Failed to serialize processed symbol info: {:?}", e);
            JobError::Failure("failed to serialize processed symbol info".to_string())
        })?;

        let path = format!("processed-symbols/{symbol_upload_id}.json");
        self.storage
            .put(&Path::from(path.as_str()), json_bytes.into())
            .await
            .map_err(|e| {
                error!("Failed to write processed symbol to S3: {:?}", e);
                JobError::Failure("failed to write processed symbol to S3".to_string())
            })?;

        Ok(())
    }

    #[instrument(skip(job, state, pg_storage))]
    pub async fn process(
        job: SymbolJob,
        state: Data<AppState>,
        pg_storage: Data<PostgresStorage<ImportSymbolJob>>,
    ) -> Result<(), JobError> {
        info!("Incoming symbol job");
        let symbol_upload_id = job.symbol_info["symbol_upload_id"]
            .as_str()
            .ok_or_else(|| {
                error!("symbol_upload_id is missing in job");
                JobError::Failure("symbol_upload_id is missing".to_string())
            })?
            .parse::<uuid::Uuid>()
            .map_err(|_| {
                error!("Invalid symbol_upload_id format in job");
                JobError::Failure("invalid symbol_upload_id format".to_string())
            })?;
        info!("Process symbol: {}", symbol_upload_id);
        let processor = SymbolProcessor::new(state.clone());
        processor
            .handle_job(symbol_upload_id, job.symbol_info.clone(), &pg_storage)
            .await?;
        info!("Successfully processed symbol for upload ID: {}", symbol_upload_id);

        Ok(())
    }
}
