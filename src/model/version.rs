use sea_orm::*;
use serde::Serialize;

use super::error::DbError;
use crate::entity;

pub use entity::version::Model as Version;

pub struct VersionRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct VersionDto {
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: uuid::Uuid,
}

impl VersionRepo {
    pub async fn create(db: &DbConn, version: VersionDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::version::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(version.name),
            hash: Set(version.hash),
            tag: Set(version.tag),
            product_id: Set(version.product_id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(model.id)
    }

    pub async fn update(
        db: &DbConn,
        id: uuid::Uuid,
        version: VersionDto,
    ) -> Result<uuid::Uuid, DbError> {
        entity::version::ActiveModel {
            id: Set(id),
            name: Set(version.name),
            ..Default::default()
        }
        .update(db)
        .await
        .map(|_| id)
        .map_err(|e| DbError::RecordNotFound("version not found".to_owned()))?;

        Ok(id)
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Version>, DbError> {
        let versions = entity::version::Entity::find().all(db).await?;
        Ok(versions)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Version, DbError> {
        let version = entity::version::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("version not found".to_owned()))?;

        Ok(version)
    }

    pub async fn get_by_name(db: &DbConn, name: &String) -> Result<Version, DbError> {
        let version = entity::version::Entity::find()
            .filter(entity::version::Column::Name.eq(name))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("version not found".to_owned()))?;

        Ok(version)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::version::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
