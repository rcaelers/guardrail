use async_trait::async_trait;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};
use crate::entity;

pub use crate::entity::sea_orm_active_enums::AnnotationKind;
pub use entity::annotation::Model as Annotation;

pub struct AnnotationRepo;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnotationDto {
    pub key: String,
    pub kind: AnnotationKind,
    pub value: String,
    pub crash_id: Uuid,
}

impl From<AnnotationDto> for entity::annotation::ActiveModel {
    fn from(annotation: AnnotationDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            key: Set(annotation.key),
            kind: Set(annotation.kind),
            value: Set(annotation.value),
            crash_id: Set(annotation.crash_id),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, AnnotationDto)> for entity::annotation::ActiveModel {
    fn from((id, annotation): (uuid::Uuid, AnnotationDto)) -> Self {
        Self {
            id: Set(id),
            ..From::from(annotation)
        }
    }
}

impl HasId for entity::annotation::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for AnnotationRepo {
    type CreateDto = AnnotationDto;
    type UpdateDto = AnnotationDto;
    type Entity = entity::annotation::Entity;
    type Repr = entity::annotation::Model;
    type ActiveModel = entity::annotation::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

#[async_trait]
impl BaseRepoWithSecondaryKey for AnnotationRepo {
    type Column = entity::annotation::Column;
    type SecondaryKeyType = String;

    fn secondary_column() -> Self::Column {
        entity::annotation::Column::Key
    }
}
