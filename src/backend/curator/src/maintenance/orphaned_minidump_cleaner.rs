use futures::stream::TryStreamExt;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::collections::{HashMap, HashSet};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{error, info};
use uuid::Uuid;

use crate::error::JobError;
use crate::state::AppState;
use common::QueryParams;
use repos::crash::CrashRepo;

pub struct OrphanedMinidumpCleaner;

impl OrphanedMinidumpCleaner {
    pub async fn run(app_state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting removal of orphaned S3 minidumps");

        let storage = app_state.storage.clone();
        let repo = &app_state.repo;

        let s3_paths = Self::get_s3_minidumps(&storage).await?;
        let db_minidumps = Self::get_database_minidumps(&repo.db).await?;
        let crash_info_minidumps = Self::get_crash_info_minidumps(&storage).await?;

        let deleted_count = Self::delete_orphaned_minidumps(
            &storage,
            &s3_paths,
            &db_minidumps,
            &crash_info_minidumps,
        )
        .await?;
        info!("Deleted {} orphaned S3 minidumps", deleted_count);

        info!("Completed removal of orphaned S3 minidumps");
        Ok(())
    }

    async fn get_s3_minidumps(storage: &dyn ObjectStore) -> Result<HashMap<Uuid, Path>, JobError> {
        let mut s3_minidump_paths_to_ids: HashMap<Uuid, Path> = HashMap::new();

        let mut minidump_stream = storage.list(Some(&Path::from("minidumps/")));
        while let Some(object_meta) = minidump_stream.try_next().await? {
            let path_str = object_meta.location.to_string();
            if let Some(name) = path_str.strip_prefix("minidumps/") {
                if name.ends_with('/') {
                    info!("Skipping directory-like path in minidumps/: {}", path_str);
                    continue;
                }
                if let Ok(uuid) = Uuid::parse_str(name) {
                    s3_minidump_paths_to_ids.insert(uuid, object_meta.location.clone());
                } else {
                    info!(
                        "Skipping non-UUID file/path in minidumps/ (or path with suffix): {}",
                        path_str
                    );
                }
            }
        }
        info!("Found {} minidumps in S3 storage", s3_minidump_paths_to_ids.len());
        Ok(s3_minidump_paths_to_ids)
    }

    async fn get_database_minidumps(db: &Surreal<Any>) -> Result<HashSet<Uuid>, JobError> {
        let crashes = CrashRepo::get_all(db, QueryParams::default()).await?;
        let db_minidump_storage_ids: HashSet<Uuid> = crashes
            .into_iter()
            .filter_map(|crash| crash.minidump)
            .collect();
        info!("Found {} minidump references in database", db_minidump_storage_ids.len());
        Ok(db_minidump_storage_ids)
    }

    async fn get_crash_info_minidumps(
        storage: &dyn ObjectStore,
    ) -> Result<HashSet<Uuid>, JobError> {
        let mut crash_info_minidump_storage_ids = HashSet::new();
        let mut crash_info_stream = storage.list(Some(&Path::from("crashes/")));

        while let Some(object_meta) = crash_info_stream.try_next().await? {
            if object_meta.location.to_string().ends_with(".json")
                && let Some(uuid) =
                    Self::extract_minidump_uuid_from_crash_info(storage, &object_meta.location)
                        .await
            {
                crash_info_minidump_storage_ids.insert(uuid);
            }
        }

        info!(
            "Found {} minidump references in S3 crash_info files",
            crash_info_minidump_storage_ids.len()
        );

        Ok(crash_info_minidump_storage_ids)
    }

    async fn extract_minidump_uuid_from_crash_info(
        storage: &dyn ObjectStore,
        location: &Path,
    ) -> Option<Uuid> {
        let bytes = match storage.get(location).await {
            Ok(get_result) => match get_result.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to read bytes from S3 object {}: {}", location, e);
                    return None;
                }
            },
            Err(e) => {
                error!("Failed to get S3 object {}: {}", location, e);
                return None;
            }
        };

        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(json_value) => {
                if let Some(minidump_val) = json_value.get("minidump")
                    && let Some(storage_id_str) =
                        minidump_val.get("storage_id").and_then(|v| v.as_str())
                {
                    if let Ok(uuid) = Uuid::parse_str(storage_id_str) {
                        return Some(uuid);
                    } else {
                        error!(
                            "Failed to parse storage_id UUID from crash_info {}: {}",
                            location, storage_id_str
                        );
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse JSON from {}: {}", location, e);
            }
        }
        None
    }

    async fn delete_orphaned_minidumps(
        storage: &dyn ObjectStore,
        s3_paths: &HashMap<Uuid, Path>,
        db_minidumps: &HashSet<Uuid>,
        crash_info_minidumps: &HashSet<Uuid>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut deleted_count = 0;

        for s3_id in s3_paths.keys() {
            if !db_minidumps.contains(s3_id)
                && !crash_info_minidumps.contains(s3_id)
                && let Some(path_to_delete) = s3_paths.get(s3_id)
            {
                info!("Deleting orphaned minidump: {}", path_to_delete);
                if let Err(e) = storage.delete(path_to_delete).await {
                    error!("Failed to delete orphaned minidump {}: {}", path_to_delete, e);
                } else {
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::PutPayload;

    #[tokio::test]
    async fn get_s3_minidumps_filters_uuid_paths() {
        let storage = object_store::memory::InMemory::new();
        let valid_id = Uuid::new_v4();
        storage
            .put(&Path::from(format!("minidumps/{valid_id}")), PutPayload::from_static(b"dump"))
            .await
            .unwrap();
        storage
            .put(&Path::from("minidumps/not-a-uuid"), PutPayload::from_static(b"dump"))
            .await
            .unwrap();
        storage
            .put(&Path::from("minidumps/directory/"), PutPayload::from_static(b""))
            .await
            .unwrap();

        let minidumps = OrphanedMinidumpCleaner::get_s3_minidumps(&storage)
            .await
            .unwrap();

        assert_eq!(minidumps.len(), 1);
        assert_eq!(minidumps.get(&valid_id), Some(&Path::from(format!("minidumps/{valid_id}"))));
    }

    #[tokio::test]
    async fn extract_minidump_uuid_from_crash_info_handles_valid_and_invalid_json() {
        let storage = object_store::memory::InMemory::new();
        let valid_id = Uuid::new_v4();
        storage
            .put(
                &Path::from("crashes/valid.json"),
                PutPayload::from(
                    serde_json::json!({"minidump": {"storage_id": valid_id.to_string()}})
                        .to_string()
                        .into_bytes(),
                ),
            )
            .await
            .unwrap();
        storage
            .put(&Path::from("crashes/invalid.json"), PutPayload::from_static(b"{"))
            .await
            .unwrap();
        storage
            .put(
                &Path::from("crashes/bad-id.json"),
                PutPayload::from_static(br#"{"minidump":{"storage_id":"not-a-uuid"}}"#),
            )
            .await
            .unwrap();

        assert_eq!(
            OrphanedMinidumpCleaner::extract_minidump_uuid_from_crash_info(
                &storage,
                &Path::from("crashes/valid.json")
            )
            .await,
            Some(valid_id)
        );
        assert!(
            OrphanedMinidumpCleaner::extract_minidump_uuid_from_crash_info(
                &storage,
                &Path::from("crashes/invalid.json")
            )
            .await
            .is_none()
        );
        assert!(
            OrphanedMinidumpCleaner::extract_minidump_uuid_from_crash_info(
                &storage,
                &Path::from("crashes/bad-id.json")
            )
            .await
            .is_none()
        );
        assert!(
            OrphanedMinidumpCleaner::extract_minidump_uuid_from_crash_info(
                &storage,
                &Path::from("crashes/missing.json")
            )
            .await
            .is_none()
        );
    }

    #[tokio::test]
    async fn delete_orphaned_minidumps_keeps_referenced_files() {
        let storage = object_store::memory::InMemory::new();
        let orphan = Uuid::new_v4();
        let in_db = Uuid::new_v4();
        let in_crash_info = Uuid::new_v4();
        let s3_paths = HashMap::from([
            (orphan, Path::from(format!("minidumps/{orphan}"))),
            (in_db, Path::from(format!("minidumps/{in_db}"))),
            (in_crash_info, Path::from(format!("minidumps/{in_crash_info}"))),
        ]);
        for path in s3_paths.values() {
            storage
                .put(path, PutPayload::from_static(b"dump"))
                .await
                .unwrap();
        }

        let deleted = OrphanedMinidumpCleaner::delete_orphaned_minidumps(
            &storage,
            &s3_paths,
            &HashSet::from([in_db]),
            &HashSet::from([in_crash_info]),
        )
        .await
        .unwrap();

        assert_eq!(deleted, 1);
        assert!(
            storage
                .get(&Path::from(format!("minidumps/{orphan}")))
                .await
                .is_err()
        );
        assert!(
            storage
                .get(&Path::from(format!("minidumps/{in_db}")))
                .await
                .is_ok()
        );
        assert!(
            storage
                .get(&Path::from(format!("minidumps/{in_crash_info}")))
                .await
                .is_ok()
        );
    }
}
