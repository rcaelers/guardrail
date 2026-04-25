use apalis::prelude::Data;
use bytes::Bytes;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use serde_json::Value;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info, instrument};

use crate::error::JobError;
use crate::jobs::ImportCrashJob;
use crate::state::AppState;
use data::{
    annotation::NewAnnotation, attachment::NewAttachment, crash::NewCrash,
    crash_group::NewCrashGroup,
};
use repos::{
    Repo, annotation::AnnotationsRepo, attachment::AttachmentsRepo, crash::CrashRepo,
    crash_group::CrashGroupRepo, product::ProductRepo,
};

pub struct ImportCrashProcessor {
    storage: Arc<dyn ObjectStore>,
    repo: Repo,
}

impl ImportCrashProcessor {
    pub fn new(s: Data<AppState>) -> ImportCrashProcessor {
        let storage = s.storage.clone();
        let repo = s.repo.clone();

        ImportCrashProcessor { storage, repo }
    }

    #[instrument(skip(self))]
    async fn get_processed_crash(&self, crash_id: &str) -> Result<Bytes, JobError> {
        let path = format!("processed-crashes/{crash_id}.json");
        let object = self
            .storage
            .get(&Path::from(path.as_str()))
            .await
            .map_err(|err| {
                error!("Failed to get processed crash object from {}: {err}", path);
                JobError::Failure("Failed to retrieve processed crash".to_string())
            })?;
        info!("Got processed crash object: {:?}", object);
        let data = object.bytes().await.map_err(|err| {
            error!("Failed to read processed crash object: {err}");
            JobError::Failure("Failed to retrieve processed crash".to_string())
        })?;
        Ok(data)
    }

    #[instrument(skip(self), fields(crash_id = %crash_id))]
    async fn handle_job(&self, crash_id: String) -> Result<(), JobError> {
        info!("ImportCrashProcessor handling job: {}", crash_id);

        let data = self.get_processed_crash(&crash_id).await?;
        let processed: Value = serde_json::from_slice(&data).map_err(|e| {
            error!("Failed to parse processed crash JSON: {:?}", e);
            JobError::Failure("failed to parse processed crash JSON".to_string())
        })?;

        let crash_info = processed["crash_info"].clone();
        let report = processed["report"].clone();

        Self::create_crash(&self.repo.db, &crash_id, crash_info, report).await?;
        info!("Imported crash report with ID: {:?}", crash_id);

        // Clean up processed crash file after successful import
        self.cleanup_processed_crash(crash_id).await;

        Ok(())
    }

    #[instrument(skip(db, crash_info, report))]
    async fn create_crash(
        db: &Surreal<Any>,
        crash_id: &str,
        crash_info: serde_json::Value,
        report: serde_json::Value,
    ) -> Result<String, JobError> {
        let product_id = crash_info["product_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("product_id is missing".to_string()))?
            .to_string();

        let minidump_id = crash_info["minidump"]["storage_id"]
            .as_str()
            .ok_or_else(|| {
                error!("Minidump ID is missing in job");
                JobError::Failure("minidump_id is missing".to_string())
            })?
            .parse::<uuid::Uuid>()
            .map_err(|_| {
                error!("Invalid minidump ID format in job");
                JobError::Failure("invalid minidump_id format".to_string())
            })?;

        let product = ProductRepo::get_by_id(db, &product_id)
            .await
            .map_err(|_| JobError::Failure(format!("failed to get product {product_id}")))?
            .ok_or_else(|| JobError::Failure(format!("no such product {product_id}")))?;

        let fingerprint = crash_info["fingerprint"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let group_id = if let Some(fp) = fingerprint.as_deref() {
            match Self::find_or_create_crash_group(db, &product.id, fp).await {
                Ok(id) => Some(id),
                Err(e) => {
                    error!(fingerprint = %fp, error = ?e, "Failed to assign crash group; crash will be stored without one");
                    None
                }
            }
        } else {
            None
        };

        let crash = NewCrash {
            id: Some(crash_id.to_string()),
            minidump: Some(minidump_id),
            fingerprint,
            group_id,
            product_id,
            report: Some(report),
        };

        let id = CrashRepo::create(db, crash).await.map_err(|e| {
            error!("Failed to store crash report for {} ({:?})", product.name, e);
            JobError::Failure("failed to store crash report".to_string())
        })?;

        Self::create_annotations(db, &id, &product.id, &crash_info).await?;
        Self::create_attachments(db, &id, &product.id, &crash_info).await?;
        info!("Created crash report with ID: {}", id);
        Ok(id)
    }

    /// Find an existing crash group for this (product, fingerprint) pair, or create one.
    ///
    /// The unique index on `crash_groups (product_id, fingerprint)` means two concurrent
    /// workers can race on the first crash for a given fingerprint. If the CREATE is
    /// rejected by a uniqueness violation we fall back to a second find, which will
    /// succeed because the winning worker just created the row.
    #[instrument(skip(db))]
    async fn find_or_create_crash_group(
        db: &Surreal<Any>,
        product_id: &str,
        fingerprint: &str,
    ) -> Result<String, JobError> {
        if let Some(group) = CrashGroupRepo::find_by_fingerprint(db, product_id, fingerprint)
            .await
            .map_err(|e| JobError::Failure(format!("failed to query crash group: {e}")))?
        {
            CrashGroupRepo::touch(db, &group.id)
                .await
                .map_err(|e| JobError::Failure(format!("failed to update crash group: {e}")))?;
            return Ok(group.id);
        }

        match CrashGroupRepo::create(
            db,
            NewCrashGroup {
                product_id: product_id.to_string(),
                fingerprint: fingerprint.to_string(),
                signal: fingerprint.to_string(),
            },
        )
        .await
        {
            Ok(id) => Ok(id),
            Err(_) => {
                // Likely lost a race with another worker. Retry the find.
                let group =
                    CrashGroupRepo::find_by_fingerprint(db, product_id, fingerprint)
                        .await
                        .map_err(|e| {
                            JobError::Failure(format!("failed to re-query crash group: {e}"))
                        })?
                        .ok_or_else(|| {
                            JobError::Failure(
                                "crash group missing after concurrent create".to_string(),
                            )
                        })?;
                CrashGroupRepo::touch(db, &group.id)
                    .await
                    .map_err(|e| JobError::Failure(format!("failed to update crash group: {e}")))?;
                Ok(group.id)
            }
        }
    }

    #[instrument(skip(db, crash_info))]
    async fn create_annotations(
        db: &Surreal<Any>,
        crash_id: &str,
        product_id: &str,
        crash_info: &serde_json::Value,
    ) -> Result<(), JobError> {
        for (key, annotation_data) in crash_info["annotations"]
            .as_object()
            .unwrap_or(&serde_json::Map::new())
        {
            let (value, source) = match annotation_data {
                serde_json::Value::Object(obj) => {
                    let value = obj.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                        error!("Annotation value is missing for key: {}", key);
                        JobError::Failure("annotation value is missing".to_string())
                    })?;

                    let source = obj
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("submission");

                    (value, source)
                }
                _ => {
                    error!("Annotation data is not in expected format for key: {}", key);
                    return Err(JobError::Failure(
                        "annotation data must be string or structured object".to_string(),
                    ));
                }
            };

            let annotation = NewAnnotation {
                crash_id: crash_id.to_string(),
                product_id: product_id.to_string(),
                source: source.to_string(),
                key: key.to_string(),
                value: value.to_string(),
            };

            AnnotationsRepo::create(db, annotation).await.map_err(|e| {
                error!("Failed to create annotation: {:?}", e);
                JobError::Failure("failed to create annotation".to_string())
            })?;
        }

        Ok(())
    }

    #[instrument(skip(db, crash_info))]
    async fn create_attachments(
        db: &Surreal<Any>,
        crash_id: &str,
        product_id: &str,
        crash_info: &serde_json::Value,
    ) -> Result<(), JobError> {
        for attachment in crash_info["attachments"].as_array().unwrap_or(&vec![]) {
            let filename = attachment["filename"].as_str().ok_or_else(|| {
                error!("Attachment filename is missing");
                JobError::Failure("attachment filename is missing".to_string())
            })?;
            let content_type = attachment["content_type"].as_str().ok_or_else(|| {
                error!("Attachment content_type is missing");
                JobError::Failure("attachment content_type is missing".to_string())
            })?;
            let size = attachment["size"].as_u64().ok_or_else(|| {
                error!("Attachment size is missing");
                JobError::Failure("attachment size is missing".to_string())
            })?;
            let storage_path = attachment["storage_path"].as_str().ok_or_else(|| {
                error!("Attachment storage path is missing");
                JobError::Failure("attachment storage path is missing".to_string())
            })?;

            let attachment = NewAttachment {
                name: filename.to_string(),
                crash_id: crash_id.to_string(),
                product_id: product_id.to_string(),
                filename: filename.to_string(),
                mime_type: content_type.to_string(),
                storage_path: storage_path.to_string(),
                size: size as i64,
            };

            AttachmentsRepo::create(db, attachment).await.map_err(|e| {
                error!("Failed to create attachment: {:?}", e);
                JobError::Failure("failed to create attachment".to_string())
            })?;
        }
        Ok(())
    }

    #[instrument(skip(self), fields(crash_id = %crash_id))]
    async fn cleanup_processed_crash(&self, crash_id: String) {
        let path = format!("processed-crashes/{crash_id}.json");
        if let Err(e) = self.storage.delete(&Path::from(path.as_str())).await {
            error!(crash_id = %crash_id, path = %path, error = ?e, "Failed to delete processed crash file");
        } else {
            info!(crash_id = %crash_id, path = %path, "Successfully deleted processed crash file");
        }
    }

    #[instrument(skip(job, state), fields(crash_id = %job.crash_id))]
    pub async fn process(job: ImportCrashJob, state: Data<AppState>) -> Result<(), JobError> {
        info!("Incoming import crash job");
        let processor = ImportCrashProcessor::new(state.clone());
        processor.handle_job(job.crash_id.clone()).await?;
        info!("Successfully imported crash ID: {}", job.crash_id);

        Ok(())
    }
}
