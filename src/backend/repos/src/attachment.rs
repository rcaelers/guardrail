use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
};
use common::QueryParams;
use data::attachment::{Attachment, NewAttachment};

pub struct AttachmentsRepo {}

impl AttachmentsRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: uuid::Uuid,
    ) -> Result<Option<Attachment>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('attachments', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_all(
        db: &Surreal<Any>,
        params: QueryParams,
    ) -> Result<Vec<Attachment>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "name", "mime_type", "size", "filename"],
            &["name", "filename"],
        )?;

        let query = format!("SELECT *, meta::id(id) as id FROM attachments{suffix}");
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(
        db: &Surreal<Any>,
        attachment: NewAttachment,
    ) -> Result<uuid::Uuid, RepoError> {
        let id = uuid::Uuid::new_v4();
        let _: Option<serde_json::Value> = db
            .query("CREATE type::record('attachments', $id) CONTENT {
                name: $name,
                mime_type: $mime_type,
                size: $size,
                filename: $filename,
                crash_id: $crash_id,
                storage_path: $storage_path,
                product_id: $product_id,
                created_at: time::now(),
                updated_at: time::now(),
            }")
            .bind(("id", id.to_string()))
            .bind(("name", attachment.name.clone()))
            .bind(("mime_type", attachment.mime_type.clone()))
            .bind(("size", attachment.size))
            .bind(("filename", attachment.filename.clone()))
            .bind(("crash_id", attachment.crash_id))
            .bind(("storage_path", attachment.storage_path.clone()))
            .bind(("product_id", attachment.product_id))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(
        db: &Surreal<Any>,
        attachment: Attachment,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        let mut result = db
            .query("UPDATE type::record('attachments', $id) SET
                name = $name,
                mime_type = $mime_type,
                size = $size,
                filename = $filename,
                updated_at = time::now()
            RETURN meta::id(id) as id")
            .bind(("id", attachment.id.to_string()))
            .bind(("name", attachment.name.clone()))
            .bind(("mime_type", attachment.mime_type.clone()))
            .bind(("size", attachment.size))
            .bind(("filename", attachment.filename.clone()))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows.first().and_then(|r| {
            r.get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
        }))
    }

    pub async fn remove(
        db: &Surreal<Any>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        db.query("DELETE type::record('attachments', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(
        db: &Surreal<Any>,
    ) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM attachments GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }
}
