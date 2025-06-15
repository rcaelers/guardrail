use apalis::prelude::*;
use apalis_sql::postgres::PostgresStorage;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{jobs::MinidumpJob, state::AppState};

pub mod database_vacuum;
pub mod job_cleaner;
pub mod orphaned_attachment_cleaner;
pub mod orphaned_minidump_cleaner;

pub use database_vacuum::DatabaseVacuum;
pub use job_cleaner::JobCleaner;
pub use orphaned_attachment_cleaner::OrphanedAttachmentCleaner;
pub use orphaned_minidump_cleaner::OrphanedMinidumpCleaner;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceJob;

impl MaintenanceJob {
    pub async fn run_maintenance_tasks(
        _job: MaintenanceJob,
        app_state: Data<AppState>,
        pg: Data<PostgresStorage<MinidumpJob>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app_state = (*app_state).clone();
        let pg = (*pg).clone();

        Self::run_all_maintenance_tasks(app_state, &pg).await?;
        Ok(())
    }

    pub async fn run_all_maintenance_tasks(
        app_state: AppState,
        pg: &PostgresStorage<MinidumpJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting all maintenance tasks");

        if let Err(e) = DatabaseVacuum::run(pg.clone()).await {
            error!("Failed to run database vacuum: {}", e);
        }

        if let Err(e) = OrphanedMinidumpCleaner::run(&app_state).await {
            error!("Failed to remove orphaned S3 minidumps: {}", e);
        }

        if let Err(e) = OrphanedAttachmentCleaner::run(&app_state).await {
            error!("Failed to remove orphaned S3 attachments: {}", e);
        }

        if let Err(e) = JobCleaner::run(&app_state, pg).await {
            error!("Failed to run Apalis job cleaner: {}", e);
        }

        info!("Completed all maintenance tasks");
        Ok(())
    }
}
