use async_trait::async_trait;
use sea_orm::*;
use serde::Serialize;

use super::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};
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
            id: Set(uuid::Uuid::new_v4()),
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
            ..From::from(version)
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

#[async_trait]
impl BaseRepoWithSecondaryKey for VersionRepo {
    type Column = entity::version::Column;
    type SecondaryKeyType = String;

    fn secondary_column() -> Self::Column {
        entity::version::Column::Name
    }
}
