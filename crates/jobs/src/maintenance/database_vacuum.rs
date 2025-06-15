use apalis::prelude::Storage;
use apalis_sql::postgres::PostgresStorage;
use tracing::info;

use crate::jobs::MinidumpJob;

pub struct DatabaseVacuum;

impl DatabaseVacuum {
    pub async fn run(
        mut pg: PostgresStorage<MinidumpJob>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Running database vacuum");
        pg.vacuum().await?;
        Ok(())
    }
}
