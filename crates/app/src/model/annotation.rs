use super::base::HasId;
use crate::entity;

pub type Annotation = entity::annotation::Model;
pub type AnnotationCreateDto = entity::annotation::CreateModel;
pub type AnnotationUpdateDto = entity::annotation::UpdateModel;

impl HasId for entity::annotation::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}
