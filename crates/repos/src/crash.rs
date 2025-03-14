use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Crash {
    pub id: uuid::Uuid,
    pub summary: String,
    pub report: serde_json::Value,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct  NewCrash {
    pub summary: String,
    pub report: serde_json::Value,
    pub version_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{Crash, NewCrash};
    use crate::{QueryParams, Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};

    pub struct CrashRepo {}

    impl CrashRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Crash>, RepoError> {
            let row = sqlx::query_as!(
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
                let message = format!("Failed to retrieve crash {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
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

            let rows = query.fetch_all(executor).await.map_err(|err| {
                let message = format!("Failed to retrieve all crashes: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            crash: NewCrash,
        ) -> Result<uuid::Uuid, RepoError> {
            let crash_id = sqlx::query_scalar!(
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
                let message = format!("Failed to create crash: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(crash_id)
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            crash: Crash,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
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
                let message = format!("Failed to update crash: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(id)
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
                let message = format!("Failed to remove crash: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        pub async fn count(executor: impl sqlx::Executor<'_, Database = Postgres>) -> Result<i64, RepoError> {
            let count = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*)
                FROM guardrail.crashes
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to count crashes: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
