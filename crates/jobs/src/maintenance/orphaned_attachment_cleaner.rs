use futures::stream::TryStreamExt;
use object_store::{ObjectStore, path::Path};
use sqlx::Postgres;
use std::collections::{HashMap, HashSet};
use tracing::{error, info};
use uuid::Uuid;

use crate::{error::JobError, state::AppState};
use common::QueryParams;
use repos::attachment::AttachmentsRepo;

pub struct OrphanedAttachmentCleaner;

impl OrphanedAttachmentCleaner {
    pub async fn run(app_state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting removal of orphaned S3 attachments");

        let storage = app_state.storage.clone();
        let repo = &app_state.repo;

        let mut tx = repo.begin_admin().await?;

        let s3_paths = Self::get_s3_attachments(&storage).await?;
        let db_attachments = Self::get_database_attachments(&mut *tx).await?;
        let crash_info_attachments = Self::get_crash_info_attachments(&storage).await?;

        let deleted_count = Self::delete_orphaned_attachments(
            &storage,
            &s3_paths,
            &db_attachments,
            &crash_info_attachments,
        )
        .await?;
        info!("Deleted {} orphaned S3 attachments", deleted_count);

        tx.commit().await?;

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

    async fn get_database_attachments<E>(tx: &mut E) -> Result<HashSet<Uuid>, JobError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let attachments = AttachmentsRepo::get_all(tx, QueryParams::default()).await?;
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
