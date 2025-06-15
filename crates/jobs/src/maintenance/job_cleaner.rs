use apalis::prelude::*;
use apalis_sql::postgres::PostgresStorage;
use futures::stream::TryStreamExt;
use object_store::{ObjectStore, path::Path};
use std::collections::HashSet;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{error::JobError, jobs::MinidumpJob, state::AppState};

pub struct JobCleaner;

impl JobCleaner {
    pub async fn run(
        app_state: &AppState,
        storage: &PostgresStorage<MinidumpJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting Apalis job cleaner");

        let object_store = app_state.storage.clone();

        let completed_job_crash_ids = Self::get_completed_job_crash_ids(storage).await?;
        if completed_job_crash_ids.is_empty() {
            info!("No completed jobs found, nothing to clean up");
            return Ok(());
        }
        info!("Found {} crash_ids from completed jobs", completed_job_crash_ids.len());

        let deleted_count =
            Self::remove_crash_info_files(&object_store, &completed_job_crash_ids).await?;

        info!("Deleted {} crash_info files for completed jobs", deleted_count);
        info!("Completed Apalis job cleaner");
        Ok(())
    }

    async fn get_completed_job_crash_ids(
        storage: &PostgresStorage<MinidumpJob>,
    ) -> Result<HashSet<Uuid>, JobError> {
        let mut crash_ids = HashSet::new();

        let done_jobs = storage
            .list_jobs(&apalis::prelude::State::Done, 1)
            .await
            .map_err(|e| JobError::Failure(format!("Failed to list Done jobs: {e}")))?;

        for job in done_jobs {
            if let Some(crash_id) = Self::extract_crash_id_from_job(&job.args) {
                crash_ids.insert(crash_id);
            }
        }

        let killed_jobs = storage
            .list_jobs(&apalis::prelude::State::Killed, 1)
            .await
            .map_err(|e| JobError::Failure(format!("Failed to list Killed jobs: {e}")))?;

        for job in killed_jobs {
            if let Some(crash_id) = Self::extract_crash_id_from_job(&job.args) {
                crash_ids.insert(crash_id);
            }
        }

        info!("Extracted {} unique crash_ids from completed Apalis jobs", crash_ids.len());
        Ok(crash_ids)
    }

    pub fn extract_crash_id_from_job(job: &MinidumpJob) -> Option<Uuid> {
        job.crash
            .get("crash_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    }

    pub async fn remove_crash_info_files(
        storage: &dyn ObjectStore,
        crash_ids: &HashSet<Uuid>,
    ) -> Result<usize, JobError> {
        let mut deleted_count = 0;

        let mut crash_stream = storage.list(Some(&Path::from("crashes/")));

        while let Some(object_meta) = crash_stream.try_next().await? {
            let path_str = object_meta.location.to_string();

            if !path_str.ends_with(".json") {
                continue;
            }

            if let Some(filename) = path_str
                .strip_prefix("crashes/")
                .and_then(|f| f.strip_suffix(".json"))
            {
                match Uuid::parse_str(filename) {
                    Ok(crash_id) => {
                        if crash_ids.contains(&crash_id) {
                            match storage.delete(&object_meta.location).await {
                                Ok(()) => {
                                    info!("Deleted crash_info file: {}", path_str);
                                    deleted_count += 1;
                                }
                                Err(e) => {
                                    error!("Failed to delete crash_info file {}: {}", path_str, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse crash_id from filename '{}': {}", filename, e);
                    }
                }
            } else {
                warn!("Unexpected crash file path format: {}", path_str);
            }
        }

        Ok(deleted_count)
    }
}
