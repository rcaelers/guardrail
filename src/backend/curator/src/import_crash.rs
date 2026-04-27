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

        // Extract signal (exception type) from the minidump report.
        let signal = report["crash_info"]["type"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| shorten_exception_type(s))
            .unwrap_or_else(|| fingerprint.clone().unwrap_or_default());

        let group_id = if let Some(fp) = fingerprint.as_deref() {
            match Self::find_or_create_crash_group(db, &product.id, fp, &signal).await {
                Ok(id) => Some(id),
                Err(e) => {
                    error!(fingerprint = %fp, error = ?e, "Failed to assign crash group; crash will be stored without one");
                    None
                }
            }
        } else {
            None
        };

        // Enrich the report with derived fields so SurrealDB can query report.* fields.
        let enriched_report = derive_report_fields(report, &crash_info);

        let crash = NewCrash {
            id: Some(crash_id.to_string()),
            minidump: Some(minidump_id),
            fingerprint,
            group_id,
            product_id,
            report: Some(enriched_report),
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
        signal: &str,
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
                signal: signal.to_string(),
            },
        )
        .await
        {
            Ok(id) => Ok(id),
            Err(_) => {
                // Likely lost a race with another worker. Retry the find.
                let group = CrashGroupRepo::find_by_fingerprint(db, product_id, fingerprint)
                    .await
                    .map_err(|e| JobError::Failure(format!("failed to re-query crash group: {e}")))?
                    .ok_or_else(|| {
                        JobError::Failure("crash group missing after concurrent create".to_string())
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

// ---------------------------------------------------------------------------
// Helpers for deriving display fields from raw minidump-stackwalk JSON
// ---------------------------------------------------------------------------

/// Adds UI-friendly fields to the raw minidump report so that SurrealDB
/// `report.*` queries in db_api.rs resolve to real values.
fn derive_report_fields(mut report: Value, crash_info: &Value) -> Value {
    // Pull everything out as owned values first to avoid borrow conflicts.
    let exception_type = report["crash_info"]["type"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let address = report["crash_info"]["address"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let first_frame = &report["crashing_thread"]["frames"][0];
    let top_frame = first_frame["function"].as_str().unwrap_or("").to_string();
    let file = first_frame["file"].as_str().unwrap_or("").to_string();
    let line = first_frame["line"].as_u64();

    let os_name = report["system_info"]["os"].as_str().unwrap_or("").to_string();
    let os_ver = report["system_info"]["os_ver"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let exception_type_short = shorten_exception_type(&exception_type);
    let platform = derive_platform(&os_name);
    let os = if os_ver.is_empty() {
        os_name.clone()
    } else {
        format!("{os_name} {os_ver}")
    };

    let title = crash_info["fingerprint"].as_str().unwrap_or("").to_string();
    let at = crash_info["submission_timestamp"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let version = get_annotation_value(crash_info, "version")
        .unwrap_or("")
        .to_string();
    let build = get_annotation_value(crash_info, "BuildID")
        .or_else(|| get_annotation_value(crash_info, "build"))
        .unwrap_or("")
        .to_string();
    let user = get_annotation_value(crash_info, "Email")
        .or_else(|| get_annotation_value(crash_info, "user"))
        .unwrap_or("")
        .to_string();
    let commit = get_annotation_value(crash_info, "commit")
        .or_else(|| get_annotation_value(crash_info, "GitRevision"))
        .unwrap_or("")
        .to_string();

    let Some(obj) = report.as_object_mut() else {
        return report;
    };

    macro_rules! set_if_absent {
        ($key:expr, $val:expr) => {
            obj.entry($key).or_insert_with(|| Value::String($val));
        };
    }

    if !exception_type.is_empty() {
        set_if_absent!("exceptionType", exception_type);
        set_if_absent!("exceptionTypeShort", exception_type_short.clone());
        set_if_absent!("signal", exception_type_short);
    }
    if !address.is_empty() {
        set_if_absent!("address", address);
    }
    if !top_frame.is_empty() {
        set_if_absent!("topFrame", top_frame);
    }
    if !file.is_empty() {
        set_if_absent!("file", file);
    }
    if let Some(n) = line {
        obj.entry("line").or_insert_with(|| n.into());
    }
    if !os.is_empty() {
        set_if_absent!("os", os);
    }
    if !platform.is_empty() {
        set_if_absent!("platform", platform);
    }
    if !title.is_empty() {
        set_if_absent!("title", title);
    }
    if !at.is_empty() {
        set_if_absent!("at", at);
    }
    if !version.is_empty() {
        set_if_absent!("version", version);
    }
    if !build.is_empty() {
        set_if_absent!("build", build);
    }
    if !user.is_empty() {
        set_if_absent!("user", user);
    }
    if !commit.is_empty() {
        set_if_absent!("commit", commit);
    }
    obj.entry("similarity").or_insert_with(|| 1.0_f64.into());

    report
}

fn get_annotation_value<'a>(crash_info: &'a Value, key: &str) -> Option<&'a str> {
    crash_info["annotations"][key]["value"]
        .as_str()
        .or_else(|| crash_info["annotations"][key].as_str())
}

/// Produces a short signal label from a minidump exception type string.
/// "EXCEPTION_ACCESS_VIOLATION_WRITE" → "ACCESS_VIOLATION"
/// "SIGSEGV" → "SIGSEGV"
fn shorten_exception_type(exception_type: &str) -> String {
    let s = exception_type
        .strip_prefix("EXCEPTION_")
        .unwrap_or(exception_type);
    // Strip access-direction suffixes so read/write/exec variants share one label.
    let s = s
        .strip_suffix("_WRITE")
        .or_else(|| s.strip_suffix("_READ"))
        .or_else(|| s.strip_suffix("_EXEC"))
        .or_else(|| s.strip_suffix("_DEP"))
        .unwrap_or(s);
    s.to_string()
}

fn derive_platform(os_name: &str) -> String {
    let lower = os_name.to_lowercase();
    if lower.contains("windows") {
        "windows".to_string()
    } else if lower.contains("mac") || lower.contains("darwin") {
        "macos".to_string()
    } else if lower.contains("linux") || lower.contains("android") {
        "linux".to_string()
    } else {
        os_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_shorten_exception_type() {
        assert_eq!(shorten_exception_type("EXCEPTION_ACCESS_VIOLATION_WRITE"), "ACCESS_VIOLATION");
        assert_eq!(shorten_exception_type("EXCEPTION_ACCESS_VIOLATION_READ"), "ACCESS_VIOLATION");
        assert_eq!(shorten_exception_type("EXCEPTION_STACK_OVERFLOW"), "STACK_OVERFLOW");
        assert_eq!(shorten_exception_type("SIGSEGV"), "SIGSEGV");
        assert_eq!(shorten_exception_type("EXC_BAD_ACCESS"), "EXC_BAD_ACCESS");
    }

    #[test]
    fn test_derive_report_fields() {
        let report = json!({
            "crash_info": {
                "type": "EXCEPTION_ACCESS_VIOLATION_WRITE",
                "address": "0xdeadbeef",
                "crashing_thread": 0
            },
            "crashing_thread": {
                "frames": [
                    {
                        "function": "crash2",
                        "module": "crash.exe",
                        "file": "src/crash.cpp",
                        "line": 42
                    }
                ]
            },
            "system_info": {
                "os": "Windows NT",
                "os_ver": "10.0.19041.0",
                "cpu_arch": "amd64"
            }
        });
        let crash_info = json!({
            "fingerprint": "crash.exe!crash2|crash.exe!main",
            "submission_timestamp": "2024-01-01T00:00:00Z",
            "annotations": {
                "version": { "value": "1.2.3", "source": "submission" }
            }
        });

        let enriched = derive_report_fields(report, &crash_info);
        assert_eq!(enriched["exceptionType"], "EXCEPTION_ACCESS_VIOLATION_WRITE");
        assert_eq!(enriched["exceptionTypeShort"], "ACCESS_VIOLATION");
        assert_eq!(enriched["signal"], "ACCESS_VIOLATION");
        assert_eq!(enriched["topFrame"], "crash2");
        assert_eq!(enriched["file"], "src/crash.cpp");
        assert_eq!(enriched["line"], 42);
        assert_eq!(enriched["platform"], "windows");
        assert_eq!(enriched["title"], "crash.exe!crash2|crash.exe!main");
        assert_eq!(enriched["version"], "1.2.3");
        assert_eq!(enriched["similarity"], 1.0);
    }
}
