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
    use tracing::error;

    pub struct VersionRepo {}

    impl VersionRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Version>, RepoError> {
            sqlx::query_as!(
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
                error!("Failed to retrieve version {id}: {err}");
                RepoError::DatabaseError("Failed to retrieve version".to_string())
            })
        }

        pub async fn get_by_product_and_name(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            product_id: uuid::Uuid,
            name: &str,
        ) -> Result<Option<Version>, RepoError> {
            sqlx::query_as!(
                Version,
                r#"
                SELECT *
                FROM guardrail.versions
                WHERE guardrail.versions.name = $1 AND guardrail.versions.product_id = $2
            "#,
                name.to_string(),
                product_id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve version by name {name} for product {product_id}: {err}");
                RepoError::DatabaseError("Failed to retrieve version by name".to_string())
            })
        }

        pub async fn get_all_names(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<HashSet<String>, RepoError> {
            sqlx::query!(
                r#"
                SELECT name
                FROM guardrail.versions
            "#
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve all version names: {err}");
                RepoError::DatabaseError("Failed to retrieve all version names".to_string())
            })
            .map(|rows| {
                rows.into_iter()
                    .map(|row| row.name)
                    .collect::<HashSet<String>>()
            })
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

            query.fetch_all(executor).await.map_err(|err| {
                error!("Failed to retrieve all versions: {err}");
                RepoError::DatabaseError("Failed to retrieve versions".to_string())
            })
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            version: NewVersion,
        ) -> Result<uuid::Uuid, RepoError> {
            sqlx::query_scalar!(
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
                error!("Failed to create version: {err}");
                RepoError::DatabaseError("Failed to create version".to_string())
            })
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            version: Version,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            sqlx::query_scalar!(
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
                error!("Failed to update version {}: {err}", version.id);
                RepoError::DatabaseError("Failed to update version".to_string())
            })
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
                error!("Failed to remove version {id}: {err}");
                RepoError::DatabaseError("Failed to remove version".to_string())
            })?;

            Ok(())
        }

        pub async fn count(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<i64, RepoError> {
            sqlx::query_scalar!(
                r#"
                SELECT COUNT(*)
                FROM guardrail.versions
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                error!("Failed to count versions: {err}");
                RepoError::DatabaseError("Failed to count versions".to_string())
            })
            .map(|count| count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
