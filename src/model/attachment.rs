use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::error::DbError;
use crate::entity;

pub use entity::attachment::Model as Attachment;

pub struct AttachmentRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct AttachmentDto {
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub filename: String,
    pub crash_id: Uuid,
}

impl AttachmentRepo {
    pub async fn create(db: &DbConn, attachment: AttachmentDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::attachment::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(attachment.name),
            mime_type: Set(attachment.mime_type),
            size: Set(attachment.size),
            filename: Set(attachment.filename),
            crash_id: Set(attachment.crash_id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(model.id)
    }

    pub async fn update(
        db: &DbConn,
        id: uuid::Uuid,
        attachment: AttachmentDto,
    ) -> Result<uuid::Uuid, DbError> {
        entity::attachment::ActiveModel {
            id: Set(id),
            name: Set(attachment.name),
            mime_type: Set(attachment.mime_type),
            size: Set(attachment.size),
            filename: Set(attachment.filename),
            ..Default::default()
        }
        .update(db)
        .await
        .map(|_| id)
        .map_err(|e| DbError::RecordNotFound("attachment not found".to_owned()))?;

        Ok(id)
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Attachment>, DbError> {
        let attachments = entity::attachment::Entity::find().all(db).await?;
        Ok(attachments)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Attachment, DbError> {
        let attachment = entity::attachment::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("attachment not found".to_owned()))?;

        Ok(attachment)
    }

    pub async fn get_by_name(db: &DbConn, name: &String) -> Result<Attachment, DbError> {
        let attachment = entity::attachment::Entity::find()
            .filter(entity::attachment::Column::Name.eq(name))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("attachment not found".to_owned()))?;

        Ok(attachment)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::attachment::Entity::delete_by_id(id)
            .exec(db)
            .await?;
        Ok(())
    }
}
