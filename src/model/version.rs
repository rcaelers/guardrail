use async_trait::async_trait;
use sea_orm::*;
use serde::Serialize;

use super::{
    base::{BaseRepo, HasId},
    error::DbError,
};
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

impl From<VersionDto> for entity::version::ActiveModel {
    fn from(version: VersionDto) -> Self {
        Self {
            name: Set(version.name),
            hash: Set(version.hash),
            tag: Set(version.tag),
            product_id: Set(version.product_id),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, VersionDto)> for entity::version::ActiveModel {
    fn from((id, version): (uuid::Uuid, VersionDto)) -> Self {
        Self {
            id: Set(id),
            name: Set(version.name),
            hash: Set(version.hash),
            tag: Set(version.tag),
            product_id: Set(version.product_id),
            ..Default::default()
        }
    }
}

impl HasId for entity::version::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for VersionRepo {
    type CreateDto = VersionDto;
    type UpdateDto = VersionDto;
    type Entity = entity::version::Entity;
    type Repr = entity::version::Model;
    type ActiveModel = entity::version::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

impl VersionRepo {
    pub async fn get_by_name(db: &DbConn, name: &String) -> Result<Version, DbError> {
        let version = entity::version::Entity::find()
            .filter(entity::version::Column::Name.eq(name))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("version not found".to_owned()))?;

        Ok(version)
    }
}
