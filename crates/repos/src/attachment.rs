use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Attachment {
    pub id: uuid::Uuid,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewAttachment {
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{Attachment, NewAttachment};
    use crate::{QueryParams, Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};

    pub struct AttachmentRepo {}

    impl AttachmentRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Attachment>, RepoError> {
            let row = sqlx::query_as!(
                Attachment,
                r#"
                SELECT *
                FROM guardrail.attachments
                WHERE guardrail.attachments.id = $1
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
        ) -> Result<Vec<Attachment>, RepoError> {
            let mut builder = QueryBuilder::new("SELECT * from guardrail.attachments");
            Repo::build_query(
                &mut builder,
                &params,
                &["id", "name", "mime_type", "size", "filename"],
                &["name", "filename"],
            )?;

            let query = builder.build_query_as();

            let rows = query.fetch_all(executor).await.map_err(|err| {
                let message = format!("Failed to retrieve all attachments: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            attachment: NewAttachment,
        ) -> Result<uuid::Uuid, RepoError> {
            let crash_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.attachments
                  (
                    name,
                    mime_type,
                    size,
                    filename,
                    crash_id,
                    product_id
                  )
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING
                  id
            "#,
                attachment.name,
                attachment.mime_type,
                attachment.size,
                attachment.filename,
                attachment.crash_id,
                attachment.product_id
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
            attachment: Attachment,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
                r#"
                UPDATE guardrail.attachments
                SET name = $1, mime_type = $2, size = $3, filename = $4
                WHERE id = $5
                RETURNING id
            "#,
                attachment.name,
                attachment.mime_type,
                attachment.size,
                attachment.filename,
                attachment.id,
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
                DELETE FROM guardrail.attachments
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
                FROM guardrail.attachments
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to count attachments: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
