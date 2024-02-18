//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use super::sea_orm_active_enums::AnnotationKind;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, macros :: DeriveDtoModel,
)]
#[sea_orm(table_name = "annotation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub key: String,
    pub kind: AnnotationKind,
    pub value: String,
    pub crash_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::crash::Entity",
        from = "Column::CrashId",
        to = "super::crash::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Crash,
}

impl Related<super::crash::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Crash.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
