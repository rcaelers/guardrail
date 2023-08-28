use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::error::DbError;
use crate::entity;

pub use crate::entity::sea_orm_active_enums::AnnotationKind;
pub use entity::annotation::Model as Annotation;

pub struct AnnotationRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct AnnotationDto {
    pub key: String,
    pub kind: AnnotationKind,
    pub value: String,
    pub crash_id: Uuid,
}

impl AnnotationRepo {
    pub async fn create(db: &DbConn, annotation: AnnotationDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::annotation::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            key: Set(annotation.key),
            kind: Set(annotation.kind),
            value: Set(annotation.value),
            crash_id: Set(annotation.crash_id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(model.id)
    }

    pub async fn update(
        db: &DbConn,
        id: uuid::Uuid,
        annotation: AnnotationDto,
    ) -> Result<uuid::Uuid, DbError> {
        entity::annotation::ActiveModel {
            id: Set(id),
            key: Set(annotation.key),
            kind: Set(annotation.kind),
            value: Set(annotation.value),
            ..Default::default()
        }
        .update(db)
        .await
        .map(|_| id)
        .map_err(|e| DbError::RecordNotFound("annotation not found".to_owned()))?;

        Ok(id)
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Annotation>, DbError> {
        let annotations = entity::annotation::Entity::find().all(db).await?;
        Ok(annotations)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Annotation, DbError> {
        let annotation = entity::annotation::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("annotation not found".to_owned()))?;

        Ok(annotation)
    }

    pub async fn get_by_name(db: &DbConn, name: &String) -> Result<Annotation, DbError> {
        let annotation = entity::annotation::Entity::find()
            .filter(entity::annotation::Column::Key.eq(name))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("annotation not found".to_owned()))?;

        Ok(annotation)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::annotation::Entity::delete_by_id(id)
            .exec(db)
            .await?;
        Ok(())
    }
}
