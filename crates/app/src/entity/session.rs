//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.0-rc.5

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, macros :: DeriveDtoModel,
)]
#[sea_orm(table_name = "session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    pub expires_at: Option<DateTime>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub data: Vec<u8>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
