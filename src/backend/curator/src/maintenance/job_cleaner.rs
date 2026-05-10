use apalis::prelude::{ListTasks, Status};
use apalis_core::backend::Filter;
use apalis_redis::RedisStorage;
use futures::stream::TryStreamExt;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::collections::HashSet;
use tracing::{error, info};

use crate::error::JobError;
use crate::jobs::ImportCrashJob;
use crate::state::AppState;

pub struct JobCleaner;

impl JobCleaner {
    pub async fn run(
        app_state: &AppState,
        redis: &RedisStorage<ImportCrashJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        RedisStorage<ImportCrashJob>: ListTasks<ImportCrashJob>,
    {
        info!("Starting cleanup of crash info files for completed jobs");

        let storage = app_state.storage.clone();

        let mut crash_ids_to_clean = HashSet::new();

        let done_crash_ids = Self::get_crash_ids_for_status(redis, Status::Done).await?;
        crash_ids_to_clean.extend(done_crash_ids);

        let killed_crash_ids = Self::get_crash_ids_for_status(redis, Status::Killed).await?;
        crash_ids_to_clean.extend(killed_crash_ids);

        info!("Found {} crash_ids from completed jobs to clean up", crash_ids_to_clean.len());
        let deleted_count = Self::remove_crash_info_files(&storage, &crash_ids_to_clean).await?;

        info!("Deleted {} crash info files for completed jobs", deleted_count);
        info!("Completed cleanup of crash info files for completed jobs");
        Ok(())
    }

    async fn get_crash_ids_for_status(
        redis: &RedisStorage<ImportCrashJob>,
        status: Status,
    ) -> Result<HashSet<String>, JobError>
    where
        RedisStorage<ImportCrashJob>: ListTasks<ImportCrashJob>,
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

            let tasks = redis
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

    /// Extract crash_id from an ImportCrashJob
    pub fn extract_crash_id_from_job(job: &ImportCrashJob) -> Option<String> {
        Some(job.crash_id.clone())
    }

    pub async fn remove_crash_info_files(
        storage: &dyn ObjectStore,
        crash_ids: &HashSet<String>,
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
    ) -> Result<Vec<(String, Path)>, JobError> {
        let mut crash_info_files = Vec::new();

        let mut crash_info_stream = storage.list(Some(&Path::from("crashes/")));
        while let Some(object_meta) = crash_info_stream.try_next().await? {
            let path_str = object_meta.location.to_string();
            if let Some(name) = path_str.strip_prefix("crashes/")
                && let Some(crash_id) = name.strip_suffix(".json")
            {
                crash_info_files.push((crash_id.to_string(), object_meta.location.clone()));
            }
        }
        Ok(crash_info_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::PutPayload;

    #[test]
    fn extract_crash_id_from_job_returns_job_id() {
        assert_eq!(
            JobCleaner::extract_crash_id_from_job(&ImportCrashJob {
                crash_id: "crash-1".to_string(),
            }),
            Some("crash-1".to_string())
        );
    }

    #[tokio::test]
    async fn get_existing_crash_info_files_filters_crash_json_paths() {
        let storage = object_store::memory::InMemory::new();
        storage
            .put(&Path::from("crashes/crash-1.json"), PutPayload::from_static(b"{}"))
            .await
            .unwrap();
        storage
            .put(&Path::from("crashes/crash-2.txt"), PutPayload::from_static(b"{}"))
            .await
            .unwrap();
        storage
            .put(&Path::from("other/crash-3.json"), PutPayload::from_static(b"{}"))
            .await
            .unwrap();

        let files = JobCleaner::get_existing_crash_info_files(&storage)
            .await
            .unwrap();
        assert_eq!(files, vec![("crash-1".to_string(), Path::from("crashes/crash-1.json"))]);
    }

    #[tokio::test]
    async fn remove_crash_info_files_deletes_only_completed_jobs() {
        let storage = object_store::memory::InMemory::new();
        storage
            .put(&Path::from("crashes/remove-me.json"), PutPayload::from_static(b"{}"))
            .await
            .unwrap();
        storage
            .put(&Path::from("crashes/keep-me.json"), PutPayload::from_static(b"{}"))
            .await
            .unwrap();
        let crash_ids = HashSet::from(["remove-me".to_string()]);

        let deleted = JobCleaner::remove_crash_info_files(&storage, &crash_ids)
            .await
            .unwrap();

        assert_eq!(deleted, 1);
        assert!(
            storage
                .get(&Path::from("crashes/remove-me.json"))
                .await
                .is_err()
        );
        assert!(
            storage
                .get(&Path::from("crashes/keep-me.json"))
                .await
                .is_ok()
        );
    }
}
