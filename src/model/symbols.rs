use async_trait::async_trait;
use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::{error::DbError, base::{HasId, BaseRepo}};
use crate::entity;

pub use entity::symbols::Model as Symbols;

pub struct SymbolsRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct SymbolsDto {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
}


impl From<SymbolsDto> for entity::symbols::ActiveModel {
    fn from(symbols: SymbolsDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            os: Set(symbols.os),
            arch: Set(symbols.arch),
            build_id: Set(symbols.build_id),
            module_id: Set(symbols.module_id),
            file_location: Set(symbols.file_location),
            product_id: Set(symbols.product_id),
            version_id: Set(symbols.version_id),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, SymbolsDto)> for entity::symbols::ActiveModel {
    fn from((id, symbols): (uuid::Uuid, SymbolsDto)) -> Self {
        Self {
            id: Set(id),
            ..From::from(symbols)
        }
    }
}

impl HasId for entity::symbols::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for SymbolsRepo {
    type CreateDto = SymbolsDto;
    type UpdateDto = SymbolsDto;
    type Entity = entity::symbols::Entity;
    type Repr = entity::symbols::Model;
    type ActiveModel = entity::symbols::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

impl SymbolsRepo {
    pub async fn get_by_build_id(db: &DbConn, build_id: &String) -> Result<Symbols, DbError> {
        let symbols = entity::symbols::Entity::find()
            .filter(entity::symbols::Column::BuildId.eq(build_id))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("symbols not found".to_owned()))?;

        Ok(symbols)
    }
}
