use common::QueryParams;
use sqlx::{Postgres, QueryBuilder};

use crate::{
    Repo,
    error::{RepoError, handle_sql_error},
};
use data::attachment::{Attachment, NewAttachment};

pub struct AttachmentsRepo {}

impl AttachmentsRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<Attachment>, RepoError> {
        sqlx::query_as!(
            Attachment,
            r#"
                SELECT *
                FROM core.attachments
                WHERE core.attachments.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<Attachment>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from core.attachments");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "name", "mime_type", "size", "filename"],
            &["name", "filename"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        attachment: NewAttachment,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.attachments
                  (
                    name,
                    mime_type,
                    size,
                    filename,
                    crash_id,
                    storage_path,
                    product_id
                  )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING
                  id
            "#,
            attachment.name,
            attachment.mime_type,
            attachment.size,
            attachment.filename,
            attachment.crash_id,
            attachment.storage_path,
            attachment.product_id
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        attachment: Attachment,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE core.attachments
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
        .map_err(handle_sql_error)
    }

    pub async fn remove(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM core.attachments
                WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)
        .map(|_| ())
    }

    pub async fn count(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<i64, RepoError> {
        sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM core.attachments
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
