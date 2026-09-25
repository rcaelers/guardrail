use std::sync::Arc;

use apalis::prelude::*;
use apalis_redis::RedisStorage;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use common::import_failure::{
    ACTIVE_IMPORT_RETRY_SECONDS, ImportFailure, ImportFailureStatus, ImportKind,
    STALLED_IMPORT_AGE_SECONDS,
};
use common::jobs::{ImportCrashJob, ImportSymbolJob, RetryImportJob};
use tracing::{info, warn};

use crate::error::JobError;
use crate::import_failure;
use crate::state::AppState;

#[async_trait]
pub(crate) trait ImportJobQueue: Send + Sync + 'static {
    async fn enqueue(&self, kind: ImportKind, id: &str) -> Result<(), String>;
}

pub(crate) struct ValkeyImportJobQueue {
    crash: RedisStorage<ImportCrashJob>,
    symbol: RedisStorage<ImportSymbolJob>,
}

impl ValkeyImportJobQueue {
    pub(crate) fn new(
        crash: RedisStorage<ImportCrashJob>,
        symbol: RedisStorage<ImportSymbolJob>,
    ) -> Self {
        Self { crash, symbol }
    }
}

#[async_trait]
impl ImportJobQueue for ValkeyImportJobQueue {
    async fn enqueue(&self, kind: ImportKind, id: &str) -> Result<(), String> {
        match kind {
            ImportKind::Crash => self
                .crash
                .clone()
                .push(ImportCrashJob {
                    crash_id: id.to_string(),
                })
                .await
                .map(|_| ())
                .map_err(|error| format!("queue crash import: {error}")),
            ImportKind::Symbol => self
                .symbol
                .clone()
                .push(ImportSymbolJob {
                    symbol_upload_id: id.to_string(),
                })
                .await
                .map(|_| ())
                .map_err(|error| format!("queue symbol import: {error}")),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RetryImportState {
    app: AppState,
    queue: Arc<dyn ImportJobQueue>,
}

impl RetryImportState {
    pub(crate) fn new(app: AppState, queue: Arc<dyn ImportJobQueue>) -> Self {
        Self { app, queue }
    }
}

pub(crate) struct RetryImportProcessor;

impl RetryImportProcessor {
    fn safe_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 128
            && id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    }

    fn active(failure: &ImportFailure) -> bool {
        let Ok(last_failure) = chrono::DateTime::parse_from_rfc3339(&failure.last_failed_at) else {
            return false;
        };
        let grace = match failure.status {
            ImportFailureStatus::Failed => ACTIVE_IMPORT_RETRY_SECONDS,
            ImportFailureStatus::Retrying => STALLED_IMPORT_AGE_SECONDS,
        };
        last_failure.with_timezone(&Utc) > Utc::now() - Duration::seconds(grace)
    }

    #[tracing::instrument(skip(state), fields(kind = ?job.kind, import_id = %job.import_id))]
    pub(crate) async fn process(
        job: RetryImportJob,
        state: Data<RetryImportState>,
    ) -> Result<(), JobError> {
        if !Self::safe_id(&job.import_id) {
            return Err(JobError::Failure("invalid import id".to_string()));
        }

        let source_path = job.kind.source_path(&job.import_id);
        let source = import_failure::read_json(&state.app.storage, &source_path)
            .await
            .map_err(JobError::Failure)?;
        let (source_product_id, subject) =
            import_failure::context(job.kind, &job.import_id, &source)
                .map_err(JobError::Failure)?;
        if repos::record_key(&source_product_id) != repos::record_key(&job.product_id) {
            warn!(
                requested_product_id = %job.product_id,
                source_product_id,
                "Rejected retry request for an import owned by another product"
            );
            return Err(JobError::Failure("retry request product does not own import".to_string()));
        }

        let existing = import_failure::load(&state.app.storage, job.kind, &job.import_id).await;
        if existing.as_ref().is_some_and(Self::active) {
            info!("Import retry is already active; ignoring duplicate request");
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        let mut marker = ImportFailure {
            id: job.import_id.clone(),
            kind: job.kind,
            product_id: source_product_id,
            subject,
            error: existing
                .as_ref()
                .map(|failure| failure.error.clone())
                .unwrap_or_else(|| "Manual retry requested for a stalled import.".to_string()),
            attempts: existing.as_ref().map_or(0, |failure| failure.attempts),
            first_failed_at: existing
                .as_ref()
                .map(|failure| failure.first_failed_at.clone())
                .unwrap_or_else(|| now.clone()),
            last_failed_at: now,
            status: ImportFailureStatus::Retrying,
        };
        import_failure::write(&state.app.storage, &marker)
            .await
            .map_err(JobError::Failure)?;

        if let Err(error) = state.queue.enqueue(job.kind, &job.import_id).await {
            marker.status = ImportFailureStatus::Failed;
            marker.error = error.clone();
            marker.last_failed_at = Utc::now().to_rfc3339();
            if let Err(marker_error) = import_failure::write(&state.app.storage, &marker).await {
                warn!(error = %marker_error, "Failed to persist retry dispatch failure");
            }
            return Err(JobError::Failure(error));
        }

        info!("Queued retained import for curator processing");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use object_store::{ObjectStore, ObjectStoreExt, PutPayload, memory::InMemory, path::Path};

    use super::*;

    #[derive(Default)]
    struct RecordingQueue {
        jobs: Mutex<Vec<(ImportKind, String)>>,
    }

    #[async_trait]
    impl ImportJobQueue for RecordingQueue {
        async fn enqueue(&self, kind: ImportKind, id: &str) -> Result<(), String> {
            self.jobs.lock().unwrap().push((kind, id.to_string()));
            Ok(())
        }
    }

    async fn state(
        storage: Arc<dyn ObjectStore>,
        queue: Arc<dyn ImportJobQueue>,
    ) -> RetryImportState {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        let app = AppState::new(
            Arc::new(repos::Repo::new(db)),
            Arc::new(crate::settings::Settings::default()),
            storage,
        );
        RetryImportState::new(app, queue)
    }

    async fn put_source(storage: &Arc<dyn ObjectStore>) {
        storage
            .put(
                &Path::from("processed-symbols/upload-1.json"),
                PutPayload::from(
                    serde_json::to_vec(&serde_json::json!({
                        "product_id": "products:workrave",
                        "module_id": "workrave.pdb"
                    }))
                    .unwrap(),
                ),
            )
            .await
            .unwrap();
    }

    fn retry_job(product_id: &str) -> RetryImportJob {
        RetryImportJob {
            product_id: product_id.to_string(),
            kind: ImportKind::Symbol,
            import_id: "upload-1".to_string(),
        }
    }

    #[tokio::test]
    async fn curator_marks_and_dispatches_retry_requests() {
        let storage: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        put_source(&storage).await;
        let queue = Arc::new(RecordingQueue::default());
        let state = state(storage.clone(), queue.clone()).await;

        RetryImportProcessor::process(retry_job("workrave"), Data::new(state.clone()))
            .await
            .unwrap();

        assert_eq!(
            queue.jobs.lock().unwrap().as_slice(),
            &[(ImportKind::Symbol, "upload-1".to_string())]
        );
        let marker = import_failure::load(&storage, ImportKind::Symbol, "upload-1")
            .await
            .unwrap();
        assert_eq!(marker.status, ImportFailureStatus::Retrying);

        RetryImportProcessor::process(retry_job("products:workrave"), Data::new(state))
            .await
            .unwrap();
        assert_eq!(
            queue.jobs.lock().unwrap().len(),
            1,
            "an active retry request must not dispatch twice"
        );
    }

    #[tokio::test]
    async fn curator_rejects_cross_product_retry_requests() {
        let storage: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        put_source(&storage).await;
        let queue = Arc::new(RecordingQueue::default());
        let state = state(storage, queue.clone()).await;

        assert!(
            RetryImportProcessor::process(retry_job("another-product"), Data::new(state))
                .await
                .is_err()
        );
        assert!(queue.jobs.lock().unwrap().is_empty());
    }
}
