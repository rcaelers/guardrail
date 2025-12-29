use apalis::prelude::{ListTasks, Status};
use apalis_core::backend::Filter;
use futures::stream::TryStreamExt;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::collections::HashSet;
use tracing::{error, info};
use uuid::Uuid;

use crate::{error::JobError, jobs::MinidumpJob, state::AppState};

use super::NotifyPostgresStorage;

pub struct JobCleaner;

impl JobCleaner {
    pub async fn run(
        app_state: &AppState,
        pg: &NotifyPostgresStorage<MinidumpJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        NotifyPostgresStorage<MinidumpJob>: ListTasks<MinidumpJob>,
    {
        info!("Starting cleanup of crash info files for completed jobs");

        let storage = app_state.storage.clone();

        let mut crash_ids_to_clean = HashSet::new();

        let done_crash_ids = Self::get_crash_ids_for_status(pg, Status::Done).await?;
        crash_ids_to_clean.extend(done_crash_ids);

        let killed_crash_ids = Self::get_crash_ids_for_status(pg, Status::Killed).await?;
        crash_ids_to_clean.extend(killed_crash_ids);

        info!("Found {} crash_ids from completed jobs to clean up", crash_ids_to_clean.len());
        let deleted_count = Self::remove_crash_info_files(&storage, &crash_ids_to_clean).await?;

        info!("Deleted {} crash info files for completed jobs", deleted_count);
        info!("Completed cleanup of crash info files for completed jobs");
        Ok(())
    }

    async fn get_crash_ids_for_status(
        pg: &NotifyPostgresStorage<MinidumpJob>,
        status: Status,
    ) -> Result<HashSet<Uuid>, JobError>
    where
        NotifyPostgresStorage<MinidumpJob>: ListTasks<MinidumpJob>,
    {
        let mut crash_ids = HashSet::new();
        let mut page = 1u32;
        let page_size = 100u32;

        loop {
            let filter = Filter {
                status: Some(status.clone()),
                page,
                page_size: Some(page_size),
            };

            let tasks = pg
                .list_tasks("guardrail::Jobs", &filter)
                .await
                .map_err(|e| {
                    JobError::Failure(format!(
                        "Failed to list tasks with status {:?}: {}",
                        status, e
                    ))
                })?;

            if tasks.is_empty() {
                break;
            }

            for task in &tasks {
                if let Some(crash_id) = Self::extract_crash_id_from_job(&task.args) {
                    crash_ids.insert(crash_id);
                }
            }

            // If we got fewer than page_size, we've reached the end
            if tasks.len() < page_size as usize {
                break;
            }

            page += 1;
        }

        info!("Found {} crash_ids with status {:?}", crash_ids.len(), status);
        Ok(crash_ids)
    }

    /// Extract crash_id from a MinidumpJob
    pub fn extract_crash_id_from_job(job: &MinidumpJob) -> Option<Uuid> {
        job.crash
            .get("crash_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    }

    pub async fn remove_crash_info_files(
        storage: &dyn ObjectStore,
        crash_ids: &HashSet<Uuid>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut deleted_count = 0;

        let existing_files = Self::get_existing_crash_info_files(storage).await?;

        for (file_crash_id, path) in existing_files {
            if crash_ids.contains(&file_crash_id) {
                info!("Deleting crash info file for completed job: {}", path);
                if let Err(e) = storage.delete(&path).await {
                    error!("Failed to delete crash info file {}: {}", path, e);
                } else {
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    async fn get_existing_crash_info_files(
        storage: &dyn ObjectStore,
    ) -> Result<Vec<(Uuid, Path)>, JobError> {
        let mut crash_info_files = Vec::new();

        let mut crash_info_stream = storage.list(Some(&Path::from("crashes/")));
        while let Some(object_meta) = crash_info_stream.try_next().await? {
            let path_str = object_meta.location.to_string();
            if let Some(name) = path_str.strip_prefix("crashes/")
                && let Some(uuid_str) = name.strip_suffix(".json")
                && let Ok(uuid) = Uuid::parse_str(uuid_str)
            {
                crash_info_files.push((uuid, object_meta.location.clone()));
            }
        }
        Ok(crash_info_files)
    }
}
