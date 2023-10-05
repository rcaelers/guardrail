//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "product")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    #[sea_orm(unique)]
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::crash::Entity")]
    Crash,
    #[sea_orm(has_many = "super::symbols::Entity")]
    Symbols,
    #[sea_orm(has_many = "super::version::Entity")]
    Version,
}

impl Related<super::crash::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Crash.def()
    }
}

impl Related<super::symbols::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Symbols.def()
    }
}

impl Related<super::version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Version.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
