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
use repos::attachment::AttachmentsRepo;

pub struct OrphanedAttachmentCleaner;

impl OrphanedAttachmentCleaner {
    pub async fn run(app_state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting removal of orphaned S3 attachments");

        let storage = app_state.storage.clone();
        let repo = &app_state.repo;

        let s3_paths = Self::get_s3_attachments(&storage).await?;
        let db_attachments = Self::get_database_attachments(&repo.db).await?;
        let crash_info_attachments = Self::get_crash_info_attachments(&storage).await?;

        let deleted_count = Self::delete_orphaned_attachments(
            &storage,
            &s3_paths,
            &db_attachments,
            &crash_info_attachments,
        )
        .await?;
        info!("Deleted {} orphaned S3 attachments", deleted_count);

        info!("Completed removal of orphaned S3 attachments");
        Ok(())
    }

    async fn get_s3_attachments(
        storage: &dyn ObjectStore,
    ) -> Result<HashMap<Uuid, Path>, JobError> {
        let mut s3_attachment_paths_to_ids: HashMap<Uuid, Path> = HashMap::new();

        let mut attachment_stream = storage.list(Some(&Path::from("attachments/")));
        while let Some(object_meta) = attachment_stream.try_next().await? {
            let path_str = object_meta.location.to_string();
            if let Some(name) = path_str.strip_prefix("attachments/") {
                if name.ends_with('/') {
                    info!("Skipping directory-like path in attachments/: {}", path_str);
                    continue;
                }
                if let Ok(uuid) = Uuid::parse_str(name) {
                    s3_attachment_paths_to_ids.insert(uuid, object_meta.location.clone());
                } else {
                    info!(
                        "Skipping non-UUID file/path in attachments/ (or path with suffix): {}",
                        path_str
                    );
                }
            }
        }
        info!("Found {} attachments in S3 storage", s3_attachment_paths_to_ids.len());
        Ok(s3_attachment_paths_to_ids)
    }

    async fn get_database_attachments(db: &Surreal<Any>) -> Result<HashSet<Uuid>, JobError> {
        let attachments = AttachmentsRepo::get_all(db, QueryParams::default()).await?;
        let db_attachment_storage_ids: HashSet<Uuid> = attachments
            .into_iter()
            .filter_map(|attachment| {
                attachment
                    .storage_path
                    .strip_prefix("attachments/")
                    .and_then(|path| Uuid::parse_str(path).ok())
            })
            .collect();
        info!("Found {} attachment references in database", db_attachment_storage_ids.len());
        Ok(db_attachment_storage_ids)
    }

    async fn get_crash_info_attachments(
        storage: &dyn ObjectStore,
    ) -> Result<HashSet<Uuid>, JobError> {
        let mut crash_info_attachment_storage_ids = HashSet::new();
        let mut crash_info_stream = storage.list(Some(&Path::from("crashes/")));

        while let Some(object_meta) = crash_info_stream.try_next().await? {
            if object_meta.location.to_string().ends_with(".json")
                && let Some(uuids) =
                    Self::extract_attachment_uuids_from_crash_info(storage, &object_meta.location)
                        .await
            {
                crash_info_attachment_storage_ids.extend(uuids);
            }
        }
        info!(
            "Found {} attachment references in S3 crash_info files",
            crash_info_attachment_storage_ids.len()
        );

        Ok(crash_info_attachment_storage_ids)
    }

    async fn extract_attachment_uuids_from_crash_info(
        storage: &dyn ObjectStore,
        location: &Path,
    ) -> Option<Vec<Uuid>> {
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
                let mut attachment_uuids = Vec::new();

                if let Some(attachments_array) =
                    json_value.get("attachments").and_then(|v| v.as_array())
                {
                    for attachment in attachments_array {
                        if let Some(storage_id_str) =
                            attachment.get("storage_id").and_then(|v| v.as_str())
                        {
                            if let Ok(uuid) = Uuid::parse_str(storage_id_str) {
                                attachment_uuids.push(uuid);
                                continue;
                            } else {
                                error!(
                                    "Failed to parse storage_id UUID from crash_info {}: {}",
                                    location, storage_id_str
                                );
                            }
                        }
                    }
                }

                if attachment_uuids.is_empty() {
                    None
                } else {
                    Some(attachment_uuids)
                }
            }
            Err(e) => {
                error!("Failed to parse JSON from {}: {}", location, e);
                None
            }
        }
    }

    async fn delete_orphaned_attachments(
        storage: &dyn ObjectStore,
        s3_paths: &HashMap<Uuid, Path>,
        db_attachments: &HashSet<Uuid>,
        crash_info_attachments: &HashSet<Uuid>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut deleted_count = 0;

        for s3_id in s3_paths.keys() {
            if !db_attachments.contains(s3_id)
                && !crash_info_attachments.contains(s3_id)
                && let Some(path_to_delete) = s3_paths.get(s3_id)
            {
                info!("Deleting orphaned attachment: {}", path_to_delete);
                if let Err(e) = storage.delete(path_to_delete).await {
                    error!("Failed to delete orphaned attachment {}: {}", path_to_delete, e);
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
    async fn get_s3_attachments_filters_uuid_paths() {
        let storage = object_store::memory::InMemory::new();
        let valid_id = Uuid::new_v4();
        storage
            .put(
                &Path::from(format!("attachments/{valid_id}")),
                PutPayload::from_static(b"attachment"),
            )
            .await
            .unwrap();
        storage
            .put(&Path::from("attachments/not-a-uuid"), PutPayload::from_static(b"attachment"))
            .await
            .unwrap();
        storage
            .put(&Path::from("attachments/directory/"), PutPayload::from_static(b""))
            .await
            .unwrap();

        let attachments = OrphanedAttachmentCleaner::get_s3_attachments(&storage)
            .await
            .unwrap();

        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments.get(&valid_id),
            Some(&Path::from(format!("attachments/{valid_id}")))
        );
    }

    #[tokio::test]
    async fn extract_attachment_uuids_from_crash_info_handles_valid_and_invalid_json() {
        let storage = object_store::memory::InMemory::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        storage
            .put(
                &Path::from("crashes/valid.json"),
                PutPayload::from(
                    serde_json::json!({
                        "attachments": [
                            {"storage_id": first.to_string()},
                            {"storage_id": "not-a-uuid"},
                            {"storage_id": second.to_string()},
                            {}
                        ]
                    })
                    .to_string()
                    .into_bytes(),
                ),
            )
            .await
            .unwrap();
        storage
            .put(
                &Path::from("crashes/empty.json"),
                PutPayload::from_static(br#"{"attachments":[]}"#),
            )
            .await
            .unwrap();
        storage
            .put(&Path::from("crashes/invalid.json"), PutPayload::from_static(b"{"))
            .await
            .unwrap();

        let uuids = OrphanedAttachmentCleaner::extract_attachment_uuids_from_crash_info(
            &storage,
            &Path::from("crashes/valid.json"),
        )
        .await
        .unwrap();
        assert_eq!(uuids, vec![first, second]);
        assert!(
            OrphanedAttachmentCleaner::extract_attachment_uuids_from_crash_info(
                &storage,
                &Path::from("crashes/empty.json")
            )
            .await
            .is_none()
        );
        assert!(
            OrphanedAttachmentCleaner::extract_attachment_uuids_from_crash_info(
                &storage,
                &Path::from("crashes/invalid.json")
            )
            .await
            .is_none()
        );
        assert!(
            OrphanedAttachmentCleaner::extract_attachment_uuids_from_crash_info(
                &storage,
                &Path::from("crashes/missing.json")
            )
            .await
            .is_none()
        );
    }

    #[tokio::test]
    async fn delete_orphaned_attachments_keeps_referenced_files() {
        let storage = object_store::memory::InMemory::new();
        let orphan = Uuid::new_v4();
        let in_db = Uuid::new_v4();
        let in_crash_info = Uuid::new_v4();
        let s3_paths = HashMap::from([
            (orphan, Path::from(format!("attachments/{orphan}"))),
            (in_db, Path::from(format!("attachments/{in_db}"))),
            (in_crash_info, Path::from(format!("attachments/{in_crash_info}"))),
        ]);
        for path in s3_paths.values() {
            storage
                .put(path, PutPayload::from_static(b"attachment"))
                .await
                .unwrap();
        }

        let deleted = OrphanedAttachmentCleaner::delete_orphaned_attachments(
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
                .get(&Path::from(format!("attachments/{orphan}")))
                .await
                .is_err()
        );
        assert!(
            storage
                .get(&Path::from(format!("attachments/{in_db}")))
                .await
                .is_ok()
        );
        assert!(
            storage
                .get(&Path::from(format!("attachments/{in_crash_info}")))
                .await
                .is_ok()
        );
    }
}
