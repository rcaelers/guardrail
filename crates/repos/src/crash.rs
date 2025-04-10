use sqlx::{Postgres, QueryBuilder};
use tracing::error;

use common::QueryParams;
use crate::{Repo, error::RepoError};
use data::crash::{Crash, NewCrash};
pub struct CrashRepo {}

impl CrashRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<Crash>, RepoError> {
        sqlx::query_as!(
            Crash,
            r#"
                SELECT *
                FROM guardrail.crashes
                WHERE guardrail.crashes.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(|err| {
            error!("Failed to retrieve crash {id}: {err}");
            RepoError::DatabaseError("Failed to retrieve crash".to_string())
        })
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<Crash>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from guardrail.crashes");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "summary", "created_at", "updated_at"],
            &["summary"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(|err| {
            error!("Failed to retrieve all crashes: {err}");
            RepoError::DatabaseError("Failed to retrieve crashes".to_string())
        })
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        crash: NewCrash,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO guardrail.crashes
                  (
                    summary,
                    report,
                    version_id,
                    product_id
                  )
                VALUES ($1, $2, $3, $4)
                RETURNING
                  id
            "#,
            crash.summary,
            crash.report,
            crash.version_id,
            crash.product_id,
        )
        .fetch_one(executor)
        .await
        .map_err(|err| {
            error!("Failed to create crash: {err}");
            RepoError::DatabaseError("Failed to create crash".to_string())
        })
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        crash: Crash,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE guardrail.crashes
                SET summary = $1, report = $2, version_id = $3, product_id = $4
                WHERE id = $5
                RETURNING id
            "#,
            crash.summary,
            crash.report,
            crash.version_id,
            crash.product_id,
            crash.id,
        )
        .fetch_optional(executor)
        .await
        .map_err(|err| {
            error!("Failed to update crash {}: {err}", crash.id);
            RepoError::DatabaseError("Failed to update crash".to_string())
        })
    }

    pub async fn remove(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM guardrail.crashes
                WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map_err(|err| {
            error!("Failed to remove crash {id}: {err}");
            RepoError::DatabaseError("Failed to remove crash".to_string())
        })?;

        Ok(())
    }

    pub async fn count(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<i64, RepoError> {
        sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM guardrail.crashes
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(|err| {
            error!("Failed to count crashes: {err}");
            RepoError::DatabaseError("Failed to count crashes".to_string())
        })
        .map(|count| count.unwrap_or(0))
    }
}
