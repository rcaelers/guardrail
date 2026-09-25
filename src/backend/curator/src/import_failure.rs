use chrono::Utc;
use common::import_failure::{ImportFailure, ImportFailureStatus, ImportKind};
use object_store::{ObjectStore, ObjectStoreExt, PutPayload, path::Path};
use serde_json::Value;
use std::sync::Arc;

pub(crate) async fn read_json(storage: &Arc<dyn ObjectStore>, path: &str) -> Result<Value, String> {
    let object = storage
        .get(&Path::from(path))
        .await
        .map_err(|e| format!("read {path}: {e}"))?;
    let bytes = object
        .bytes()
        .await
        .map_err(|e| format!("read {path}: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse {path}: {e}"))
}

pub(crate) fn context(
    kind: ImportKind,
    id: &str,
    source: &Value,
) -> Result<(String, String), String> {
    let (product_id, subject) = match kind {
        ImportKind::Crash => (
            source["crash_info"]["product_id"].as_str(),
            source["report"]["title"].as_str().unwrap_or(id),
        ),
        ImportKind::Symbol => {
            (source["product_id"].as_str(), source["module_id"].as_str().unwrap_or(id))
        }
    };
    let product_id = product_id
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "processed import has no product_id".to_string())?;
    Ok((product_id.to_string(), subject.to_string()))
}

pub(crate) async fn load(
    storage: &Arc<dyn ObjectStore>,
    kind: ImportKind,
    id: &str,
) -> Option<ImportFailure> {
    read_json(storage, &kind.failure_path(id))
        .await
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub(crate) async fn write(
    storage: &Arc<dyn ObjectStore>,
    failure: &ImportFailure,
) -> Result<(), String> {
    let path = failure.kind.failure_path(&failure.id);
    let payload =
        serde_json::to_vec_pretty(failure).map_err(|e| format!("serialize {path}: {e}"))?;
    storage
        .put(&Path::from(path.as_str()), PutPayload::from(payload))
        .await
        .map_err(|e| format!("write {path}: {e}"))?;
    Ok(())
}

pub async fn record(
    storage: &Arc<dyn ObjectStore>,
    kind: ImportKind,
    id: &str,
    error: &str,
) -> Result<(), String> {
    let source_path = kind.source_path(id);
    let source = read_json(storage, &source_path).await?;
    let (product_id, subject) = context(kind, id, &source)?;
    let existing = load(storage, kind, id).await;
    let now = Utc::now().to_rfc3339();
    let failure = ImportFailure {
        id: id.to_string(),
        kind,
        product_id,
        subject,
        error: error.to_string(),
        attempts: existing
            .as_ref()
            .map_or(1, |value| value.attempts.saturating_add(1)),
        first_failed_at: existing
            .as_ref()
            .map(|value| value.first_failed_at.clone())
            .unwrap_or_else(|| now.clone()),
        last_failed_at: now,
        status: ImportFailureStatus::Failed,
    };
    write(storage, &failure).await
}

pub async fn clear(storage: &Arc<dyn ObjectStore>, kind: ImportKind, id: &str) {
    let path = kind.failure_path(id);
    if let Err(error) = storage.delete(&Path::from(path.as_str())).await
        && !matches!(error, object_store::Error::NotFound { .. })
    {
        tracing::error!(path, error = ?error, "Failed to clear import failure marker");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn records_attempts_and_clears_marker() {
        let storage: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        storage
            .put(
                &Path::from("processed-symbols/upload1.json"),
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

        record(&storage, ImportKind::Symbol, "upload1", "database unavailable")
            .await
            .unwrap();
        record(&storage, ImportKind::Symbol, "upload1", "session not found")
            .await
            .unwrap();

        let marker = read_json(&storage, "import-failures/symbol/upload1.json")
            .await
            .unwrap();
        let marker: ImportFailure = serde_json::from_value(marker).unwrap();
        assert_eq!(marker.product_id, "products:workrave");
        assert_eq!(marker.subject, "workrave.pdb");
        assert_eq!(marker.attempts, 2);
        assert_eq!(marker.error, "session not found");

        clear(&storage, ImportKind::Symbol, "upload1").await;
        assert!(
            storage
                .get(&Path::from("import-failures/symbol/upload1.json"))
                .await
                .is_err()
        );
    }
}
