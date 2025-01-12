use super::base::HasId;
use crate::entity;

pub type Symbols = entity::symbols::Model;
pub type SymbolsCreateDto = entity::symbols::CreateModel;
pub type SymbolsUpdateDto = entity::symbols::UpdateModel;

impl HasId for entity::symbols::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}
