use async_trait::async_trait;
use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};
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

impl From<AttachmentDto> for entity::attachment::ActiveModel {
    fn from(attachment: AttachmentDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(attachment.name),
            mime_type: Set(attachment.mime_type),
            size: Set(attachment.size),
            filename: Set(attachment.filename),
            crash_id: Set(attachment.crash_id),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, AttachmentDto)> for entity::attachment::ActiveModel {
    fn from((id, attachment): (uuid::Uuid, AttachmentDto)) -> Self {
        Self {
            id: Set(id),
            ..From::from(attachment)
        }
    }
}

impl HasId for entity::attachment::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for AttachmentRepo {
    type CreateDto = AttachmentDto;
    type UpdateDto = AttachmentDto;
    type Entity = entity::attachment::Entity;
    type Repr = entity::attachment::Model;
    type ActiveModel = entity::attachment::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

#[async_trait]
impl BaseRepoWithSecondaryKey for AttachmentRepo {
    type Column = entity::attachment::Column;
    type SecondaryKeyType = String;

    fn secondary_column() -> Self::Column {
        entity::attachment::Column::Name
    }
}
