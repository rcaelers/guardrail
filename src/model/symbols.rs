use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::error::DbError;
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

impl SymbolsRepo {
    pub async fn create(db: &DbConn, symbols: SymbolsDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::symbols::ActiveModel {
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
        .insert(db)
        .await?;

        Ok(model.id)
    }

    pub async fn update(
        db: &DbConn,
        id: uuid::Uuid,
        symbols: SymbolsDto,
    ) -> Result<uuid::Uuid, DbError> {
        entity::symbols::ActiveModel {
            id: Set(id),
            os: Set(symbols.os),
            arch: Set(symbols.arch),
            build_id: Set(symbols.build_id),
            module_id: Set(symbols.module_id),
            file_location: Set(symbols.file_location),
            ..Default::default()
        }
        .update(db)
        .await
        .map(|_| id)
        .map_err(|e| DbError::RecordNotFound("symbols not found".to_owned()))?;

        Ok(id)
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Symbols>, DbError> {
        let symbolss = entity::symbols::Entity::find().all(db).await?;
        Ok(symbolss)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Symbols, DbError> {
        let symbols = entity::symbols::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("symbols not found".to_owned()))?;

        Ok(symbols)
    }

    pub async fn get_by_build_id(db: &DbConn, build_id: &String) -> Result<Symbols, DbError> {
        let symbols = entity::symbols::Entity::find()
            .filter(entity::symbols::Column::BuildId.eq(build_id))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("symbols not found".to_owned()))?;

        Ok(symbols)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::symbols::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
