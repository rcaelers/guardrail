use apalis::prelude::Data;
use bytes::Bytes;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use serde_json::Value;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info, instrument};

use crate::error::JobError;
use crate::jobs::ImportSymbolJob;
use crate::state::AppState;
use data::symbols::NewSymbols;
use repos::symbols::SymbolsRepo;

pub struct ImportSymbolProcessor {
    storage: Arc<dyn ObjectStore>,
    repo: repos::Repo,
}

impl ImportSymbolProcessor {
    pub fn new(s: Data<AppState>) -> ImportSymbolProcessor {
        ImportSymbolProcessor {
            storage: s.storage.clone(),
            repo: s.repo.clone(),
        }
    }

    #[instrument(skip(self))]
    async fn get_processed_symbol(&self, symbol_upload_id: uuid::Uuid) -> Result<Bytes, JobError> {
        let path = format!("processed-symbols/{symbol_upload_id}.json");
        let object = self
            .storage
            .get(&Path::from(path.as_str()))
            .await
            .map_err(|err| {
                error!("Failed to get processed symbol object from {}: {err}", path);
                JobError::Failure("Failed to retrieve processed symbol".to_string())
            })?;
        let data = object.bytes().await.map_err(|err| {
            error!("Failed to read processed symbol object: {err}");
            JobError::Failure("Failed to read processed symbol".to_string())
        })?;
        Ok(data)
    }

    #[instrument(skip(self), fields(symbol_upload_id = %symbol_upload_id))]
    async fn handle_job(&self, symbol_upload_id: uuid::Uuid) -> Result<(), JobError> {
        info!("ImportSymbolProcessor handling job: {}", symbol_upload_id);

        let data = self.get_processed_symbol(symbol_upload_id).await?;
        let symbol_info: Value = serde_json::from_slice(&data).map_err(|e| {
            error!("Failed to parse processed symbol JSON: {:?}", e);
            JobError::Failure("failed to parse processed symbol JSON".to_string())
        })?;

        Self::create_symbol(&self.repo.db, &symbol_info).await?;
        info!("Imported symbol metadata for upload: {:?}", symbol_upload_id);

        self.cleanup_processed_symbol(symbol_upload_id).await;

        Ok(())
    }

    #[instrument(skip(db, symbol_info))]
    async fn create_symbol(
        db: &Surreal<Any>,
        symbol_info: &serde_json::Value,
    ) -> Result<uuid::Uuid, JobError> {
        let product_id = symbol_info["product_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("product_id is missing".to_string()))?
            .parse::<uuid::Uuid>()
            .map_err(|_| JobError::Failure("invalid product_id format".to_string()))?;

        let os = symbol_info["os"]
            .as_str()
            .ok_or_else(|| JobError::Failure("os is missing".to_string()))?
            .to_string();
        let arch = symbol_info["arch"]
            .as_str()
            .ok_or_else(|| JobError::Failure("arch is missing".to_string()))?
            .to_string();
        let build_id = symbol_info["build_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("build_id is missing".to_string()))?
            .to_string();
        let module_id = symbol_info["module_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("module_id is missing".to_string()))?
            .to_string();
        let storage_path = symbol_info["storage_path"]
            .as_str()
            .ok_or_else(|| JobError::Failure("storage_path is missing".to_string()))?
            .to_string();

        let new_symbols = NewSymbols {
            os,
            arch,
            build_id,
            module_id,
            storage_path,
            product_id,
        };

        let id = SymbolsRepo::create(db, new_symbols).await.map_err(|e| {
            error!("Failed to store symbol metadata: {:?}", e);
            JobError::Failure("failed to store symbol metadata".to_string())
        })?;

        info!("Created symbol record with ID: {}", id);
        Ok(id)
    }

    #[instrument(skip(self), fields(symbol_upload_id = %symbol_upload_id))]
    async fn cleanup_processed_symbol(&self, symbol_upload_id: uuid::Uuid) {
        let path = format!("processed-symbols/{symbol_upload_id}.json");
        if let Err(e) = self.storage.delete(&Path::from(path.as_str())).await {
            error!(symbol_upload_id = %symbol_upload_id, path = %path, error = ?e, "Failed to delete processed symbol file");
        } else {
            info!(symbol_upload_id = %symbol_upload_id, path = %path, "Successfully deleted processed symbol file");
        }
    }

    #[instrument(skip(job, state), fields(symbol_upload_id = %job.symbol_upload_id))]
    pub async fn process(job: ImportSymbolJob, state: Data<AppState>) -> Result<(), JobError> {
        info!("Incoming import symbol job");
        let processor = ImportSymbolProcessor::new(state.clone());
        processor.handle_job(job.symbol_upload_id).await?;
        info!("Successfully imported symbol upload: {}", job.symbol_upload_id);

        Ok(())
    }
}
