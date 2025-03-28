use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Symbols {
    pub id: uuid::Uuid,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    pub product_id: uuid::Uuid,
    pub version_id: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewSymbols {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    pub product_id: uuid::Uuid,
    pub version_id: uuid::Uuid,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{NewSymbols, Symbols};
    use crate::{QueryParams, Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};

    pub struct SymbolsRepo {}

    impl SymbolsRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Symbols>, RepoError> {
            let row = sqlx::query_as!(
                Symbols,
                r#"
                SELECT *
                FROM guardrail.symbols
                WHERE guardrail.symbols.id = $1
            "#,
                id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve attachment {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_all(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            params: QueryParams,
        ) -> Result<Vec<Symbols>, RepoError> {
            let mut builder = QueryBuilder::new("SELECT * from guardrail.symbols");
            Repo::build_query(
                &mut builder,
                &params,
                &["id", "os", "arch", "build_id", "module_id", "file_location"],
                &["os", "arch", "build_id", "module_id"],
            )?;

            let query = builder.build_query_as();

            let rows = query.fetch_all(executor).await.map_err(|err| {
                let message = format!("Failed to retrieve all symbols: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            attachment: NewSymbols,
        ) -> Result<uuid::Uuid, RepoError> {
            let crash_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.symbols
                  (
                    os,
                    arch,
                    build_id,
                    module_id,
                    file_location,
                    product_id,
                    version_id
                  )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING
                  id
            "#,
                attachment.os,
                attachment.arch,
                attachment.build_id,
                attachment.module_id,
                attachment.file_location,
                attachment.product_id,
                attachment.version_id
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create attachment: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(crash_id)
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            attachment: Symbols,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
                r#"
                UPDATE guardrail.symbols
                SET os = $1, arch = $2, build_id = $3, module_id = $4, file_location = $5
                WHERE id = $6
                RETURNING id
            "#,
                attachment.os,
                attachment.arch,
                attachment.build_id,
                attachment.module_id,
                attachment.file_location,
                attachment.id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to update attachment: {err}");
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
                DELETE FROM guardrail.symbols
                WHERE id = $1
            "#,
                id
            )
            .execute(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to remove attachment: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        pub async fn count(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<i64, RepoError> {
            let count = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*)
                FROM guardrail.symbols
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to count symbols: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
