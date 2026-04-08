use apalis::prelude::ListTasks;
use apalis_redis::RedisStorage;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::jobs::ImportCrashJob;
use crate::state::AppState;

pub mod job_cleaner;
pub mod orphaned_attachment_cleaner;
pub mod orphaned_minidump_cleaner;

pub use job_cleaner::JobCleaner;
pub use orphaned_attachment_cleaner::OrphanedAttachmentCleaner;
pub use orphaned_minidump_cleaner::OrphanedMinidumpCleaner;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceJob;

impl MaintenanceJob {
    pub async fn run_all_maintenance_tasks(
        app_state: &AppState,
        redis: &RedisStorage<ImportCrashJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        RedisStorage<ImportCrashJob>: ListTasks<ImportCrashJob>,
    {
        info!("Starting all maintenance tasks");

        if let Err(e) = OrphanedMinidumpCleaner::run(app_state).await {
            error!("Failed to remove orphaned S3 minidumps: {}", e);
        }

        if let Err(e) = OrphanedAttachmentCleaner::run(app_state).await {
            error!("Failed to remove orphaned S3 attachments: {}", e);
        }

        if let Err(e) = JobCleaner::run(app_state, redis).await {
            error!("Failed to run Apalis job cleaner: {}", e);
        }

        info!("Completed all maintenance tasks");
        Ok(())
    }
}
