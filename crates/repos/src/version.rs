use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Version {
    pub id: uuid::Uuid,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewVersion {
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: uuid::Uuid,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{NewVersion, Version};
    use crate::{QueryParams, Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};
    use std::collections::HashSet;

    pub struct VersionRepo {}

    impl VersionRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Version>, RepoError> {
            let row = sqlx::query_as!(
                Version,
                r#"
                SELECT *
                FROM guardrail.versions
                WHERE guardrail.versions.id = $1
            "#,
                id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve version {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_by_name(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            email: &str,
        ) -> Result<Option<Version>, RepoError> {
            let row = sqlx::query_as!(
                Version,
                r#"
                SELECT *
                FROM guardrail.versions
                WHERE guardrail.versions.name = $1
            "#,
                email.to_string()
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve version by email: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_all_names(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<HashSet<String>, RepoError> {
            let rows = sqlx::query!(
                r#"
                SELECT name
                FROM guardrail.versions
            "#
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve all version names: {err}");
                RepoError::DatabaseError(message)
            })?
            .into_iter()
            .map(|row| row.name)
            .collect::<HashSet<String>>();

            Ok(rows)
        }

        pub async fn get_all(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            params: QueryParams,
        ) -> Result<Vec<Version>, RepoError> {
            let mut builder = QueryBuilder::new("SELECT * from guardrail.versions");
            Repo::build_query(
                &mut builder,
                &params,
                &["id", "name", "hash", "tag", "created_at", "updated_at"],
                &["name"],
            )?;

            let query = builder.build_query_as();

            let rows = query.fetch_all(executor).await.map_err(|err| {
                let message = format!("Failed to retrieve all versions: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            version: NewVersion,
        ) -> Result<uuid::Uuid, RepoError> {
            let version_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.versions
                  (
                    name,
                    hash,
                    tag,
                    product_id
                  )
                VALUES ($1, $2, $3, $4)
                RETURNING
                  id
            "#,
                version.name,
                version.hash,
                version.tag,
                version.product_id
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create version: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(version_id)
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            version: Version,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
                r#"
                UPDATE guardrail.versions
                SET name = $1, tag = $2, hash = $3, product_id = $4
                WHERE id = $5
                RETURNING id
            "#,
                version.name,
                version.tag,
                version.hash,
                version.product_id,
                version.id,
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to update version: {err}");
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
                DELETE FROM guardrail.versions
                WHERE id = $1
            "#,
                id
            )
            .execute(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to remove version: {err}");
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
                FROM guardrail.versions
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to count versions: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
