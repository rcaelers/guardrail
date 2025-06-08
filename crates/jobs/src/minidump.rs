use apalis::prelude::{Context, Data, Worker};
use bytes::Bytes;
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::Symbolizer;
use object_store::{ObjectStore, path::Path};
use serde_json::Value;
use sqlx::Postgres;
use std::sync::Arc;
use tracing::{error, info};

use crate::{
    error::JobError,
    jobs::MinidumpJob,
    signature_generator::{SignatureGenerator, SignatureGeneratorConfig},
    state::AppState,
    symbol_provider::s3_symbol_supplier,
};
use data::{annotation::NewAnnotation, attachment::NewAttachment, crash::NewCrash};
use repos::{
    Repo, annotation::AnnotationsRepo, attachment::AttachmentsRepo, crash::CrashRepo,
    product::ProductRepo,
};

pub struct MinidumpProcessor {
    storage: Arc<dyn ObjectStore>,
    repo: Repo,
    signature_generator: SignatureGenerator,
}

impl MinidumpProcessor {
    pub fn new(s: Data<AppState>) -> MinidumpProcessor {
        let storage = s.storage.clone();
        let repo = s.repo.clone();

        let config = SignatureGeneratorConfig {
            end_patterns: s
                .settings
                .job_server
                .end_patterns
                .clone()
                .unwrap_or_default(),
            skip_patterns: s
                .settings
                .job_server
                .skip_patterns
                .clone()
                .unwrap_or_default(),
            delimiter: s
                .settings
                .job_server
                .delimiter
                .clone()
                .unwrap_or("|".to_string()),
            maximum_frame_count: s.settings.job_server.maximum_frame_count.unwrap_or(20),
        };

        let signature_generator = SignatureGenerator::new(config).unwrap();

        MinidumpProcessor {
            storage,
            repo,
            signature_generator,
        }
    }

    async fn generate_signature(&self, crash_info: &serde_json::Value) -> Result<String, JobError> {
        let crashing_thread = crash_info
            .get("crashing_thread")
            .ok_or_else(|| JobError::Failure("Failed to get crashing thread".to_string()))?;

        let signature = self
            .signature_generator
            .generate(crashing_thread)
            .map_err(|e| JobError::Failure(format!("Failed to generate signature: {e}")))?;

        Ok(signature)
    }

    async fn handle_job(
        &self,
        crash_id: uuid::Uuid,
        mut crash_info: serde_json::Value,
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
        let report: Value = serde_json::from_slice(&json_output).map_err(|e| {
            error!("Failed to parse minidump json: {:?}", e);
            JobError::Failure("failed to parse minidump json".to_string())
        })?;

        let signature = self.generate_signature(&report).await?;
        crash_info["signature"] = Value::String(signature);

        Self::create_crash(&mut *tx, crash_id, crash_info, report).await?;
        info!("Updated crash report with ID: {:?}", crash_id);

        tx.commit().await.map_err(|e| {
            error!(error = ?e, "Failed to commit transaction");
            JobError::Failure("failed to commit transaction".to_string())
        })?;
        Ok(())
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

    async fn create_crash<E>(
        tx: &mut E,
        crash_id: uuid::Uuid,
        crash_info: serde_json::Value,
        report: serde_json::Value,
    ) -> Result<uuid::Uuid, JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let product_id = crash_info["product_id"]
            .as_str()
            .ok_or_else(|| JobError::Failure("product_id is missing".to_string()))?
            .parse::<uuid::Uuid>()
            .map_err(|_| JobError::Failure("invalid product_id format".to_string()))?;

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

        let crash = NewCrash {
            id: Some(crash_id),
            minidump: Some(minidump_id),
            signature: crash_info["signature"].as_str().map(|s| s.to_string()),
            product_id,
            report: Some(report),
        };

        let product = ProductRepo::get_by_id(&mut *tx, product_id)
            .await
            .map_err(|_| JobError::Failure(format!("failed to get product {product_id}")))?
            .ok_or_else(|| JobError::Failure(format!("no such product {product_id}")))?;

        let id = CrashRepo::create(&mut *tx, crash).await.map_err(|e| {
            error!("Failed to store crash report for {} ({:?})", product.name, e);
            JobError::Failure("failed to store crash report".to_string())
        })?;

        Self::create_annotations(&mut *tx, id, product.id, &crash_info).await?;
        Self::create_attachments(&mut *tx, id, product.id, &crash_info).await?;
        info!("Created crash report with ID: {}", id);
        Ok(id)
    }

    async fn create_annotations<E>(
        tx: &mut E,
        crash_id: uuid::Uuid,
        product_id: uuid::Uuid,
        crash_info: &serde_json::Value,
    ) -> Result<(), JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
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
                crash_id,
                product_id,
                source: source.to_string(),
                key: key.to_string(),
                value: value.to_string(),
            };

            AnnotationsRepo::create(&mut *tx, annotation)
                .await
                .map_err(|e| {
                    error!("Failed to create annotation: {:?}", e);
                    JobError::Failure("failed to create annotation".to_string())
                })?;
        }

        Ok(())
    }

    async fn create_attachments<E>(
        tx: &mut E,
        crash_id: uuid::Uuid,
        product_id: uuid::Uuid,
        crash_info: &serde_json::Value,
    ) -> Result<(), JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
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
                crash_id,
                product_id,
                filename: filename.to_string(),
                mime_type: content_type.to_string(),
                storage_path: storage_path.to_string(),
                size: size as i64,
            };

            AttachmentsRepo::create(&mut *tx, attachment)
                .await
                .map_err(|e| {
                    error!("Failed to create attachment: {:?}", e);
                    JobError::Failure("failed to create attachment".to_string())
                })?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_annotation_processing() {
        // Test data with both submission annotations and script annotations
        let crash_info = json!({
            "annotations": {
                "product_version": "1.0.0",
                "user_email": "test@example.com",
                "submission_notes": "App crashed on startup"
            },
            "script_annotations": {
                "script_classification": "access_violation",
                "script_known_issue": "ISSUE-123",
                "product_version": "1.0.0-debug"  // Same key as submission, different value
            }
        });

        // Verify that we can extract both annotation types
        let submission_annotations = crash_info["annotations"].as_object().unwrap();
        let script_annotations = crash_info["script_annotations"].as_object().unwrap();

        // Verify submission annotations
        assert_eq!(
            submission_annotations
                .get("product_version")
                .unwrap()
                .as_str()
                .unwrap(),
            "1.0.0"
        );
        assert_eq!(
            submission_annotations
                .get("user_email")
                .unwrap()
                .as_str()
                .unwrap(),
            "test@example.com"
        );
        assert_eq!(
            submission_annotations
                .get("submission_notes")
                .unwrap()
                .as_str()
                .unwrap(),
            "App crashed on startup"
        );

        // Verify script annotations
        assert_eq!(
            script_annotations
                .get("script_classification")
                .unwrap()
                .as_str()
                .unwrap(),
            "access_violation"
        );
        assert_eq!(
            script_annotations
                .get("script_known_issue")
                .unwrap()
                .as_str()
                .unwrap(),
            "ISSUE-123"
        );
        assert_eq!(
            script_annotations
                .get("product_version")
                .unwrap()
                .as_str()
                .unwrap(),
            "1.0.0-debug"
        );

        // Verify that the same key can have different values in different sources
        assert_ne!(
            submission_annotations
                .get("product_version")
                .unwrap()
                .as_str()
                .unwrap(),
            script_annotations
                .get("product_version")
                .unwrap()
                .as_str()
                .unwrap()
        );
    }

    #[test]
    fn test_missing_annotations() {
        // Test with missing annotation fields
        let crash_info = json!({
            "annotations": {
                "product_version": "1.0.0"
            }
            // No script_annotations field
        });

        let submission_annotations = crash_info["annotations"].as_object().unwrap();
        let script_annotations = crash_info["script_annotations"].as_object();

        assert!(submission_annotations.contains_key("product_version"));
        assert!(script_annotations.is_none()); // Should handle missing script_annotations gracefully
    }

    #[test]
    fn test_empty_annotations() {
        // Test with empty annotation objects
        let crash_info = json!({
            "annotations": {},
            "script_annotations": {}
        });

        let submission_annotations = crash_info["annotations"].as_object().unwrap();
        let script_annotations = crash_info["script_annotations"].as_object().unwrap();

        assert!(submission_annotations.is_empty());
        assert!(script_annotations.is_empty());
    }
}
