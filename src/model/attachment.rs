use super::base::HasId;
use crate::entity;

pub type Attachment = entity::attachment::Model;
pub type AttachmentCreateDto = entity::attachment::CreateModel;
pub type AttachmentUpdateDto = entity::attachment::UpdateModel;

impl HasId for entity::attachment::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}
