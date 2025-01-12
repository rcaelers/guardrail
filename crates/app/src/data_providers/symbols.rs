use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::prelude::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::vec;
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use sea_orm::*;
    use sea_query::Expr;
    use entities::entity;
    use crate::data::{
        add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
    };
    use crate::auth::AuthenticatedUser;
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct SymbolsRow {
    pub id: Uuid,
    pub product: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
    #[table(skip)]
    pub version_id: Option<Uuid>,
}

#[cfg(feature = "ssr")]
#[derive(FromQueryResult, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Symbols {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Symbols {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
}

#[cfg(feature = "ssr")]
impl EntityInfo for entity::symbols::Entity {
    type View = Symbols;

    fn filter_column() -> Self::Column {
        entity::symbols::Column::BuildId
    }

    fn index_to_column(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::symbols::Column::Id),
            1 => Some(entity::symbols::Column::Os),
            2 => Some(entity::symbols::Column::Arch),
            3 => Some(entity::symbols::Column::BuildId),
            4 => Some(entity::symbols::Column::ModuleId),
            5 => Some(entity::symbols::Column::FileLocation),
            6 => Some(entity::symbols::Column::CreatedAt),
            7 => Some(entity::symbols::Column::UpdatedAt),
            _ => None,
        }
    }

    fn extend_query_for_view(query: Select<Self>) -> Select<Self> {
        query
            .join(JoinType::LeftJoin, entity::symbols::Relation::Product.def())
            .join(JoinType::LeftJoin, entity::symbols::Relation::Version.def())
            .column_as(entity::product::Column::Name, "product")
            .column_as(entity::version::Column::Name, "version")
    }

    fn get_product_query(
        _user: &AuthenticatedUser,
        data: &Self::View,
    ) -> Option<Select<entity::product::Entity>> {
        let query = entity::product::Entity::find().filter(
            Expr::col((entity::product::Entity, entity::product::Column::Id)).eq(data.product_id),
        );
        Some(query)
    }

    fn id_to_column(id_name: String) -> Option<Self::Column> {
        match id_name.as_str() {
            "product_id" => Some(entity::symbols::Column::ProductId),
            "version_id" => Some(entity::symbols::Column::VersionId),
            _ => None,
        }
    }
}
impl From<Symbols> for SymbolsRow {
    fn from(symbols: Symbols) -> Self {
        Self {
            id: symbols.id,
            os: symbols.os,
            arch: symbols.arch,
            build_id: symbols.build_id,
            module_id: symbols.module_id,
            file_location: symbols.file_location,
            created_at: symbols.created_at,
            updated_at: symbols.updated_at,
            product_id: Some(symbols.product_id),
            version_id: Some(symbols.version_id),
            product: symbols.product,
            version: symbols.version,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::symbols::Model> for Symbols {
    fn from(model: entity::symbols::Model) -> Self {
        Self {
            id: model.id,
            os: model.os,
            arch: model.arch,
            build_id: model.build_id,
            module_id: model.module_id,
            file_location: model.file_location,
            created_at: model.created_at,
            updated_at: model.updated_at,
            product_id: model.product_id,
            version_id: model.version_id,
            product: "".to_string(),
            version: "".to_string(),
        }
    }
}

#[cfg(feature = "ssr")]
impl crate::data::MyIntoActiveModel<entities::entity::symbols::ActiveModel> for Symbols {
    fn into_active_model(self) -> entities::entity::symbols::ActiveModel {
        entities::entity::symbols::ActiveModel {
            id: Set(self.id),
            os: Set(self.os),
            arch: Set(self.arch),
            build_id: Set(self.build_id),
            module_id: Set(self.module_id),
            file_location: Set(self.file_location),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
            product_id: Set(self.product_id),
            version_id: Set(self.version_id),
        }
    }
}

impl ExtraRowTrait for SymbolsRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.build_id.clone()
    }
}

#[server]
pub async fn symbols_get(id: Uuid) -> Result<Symbols, ServerFnError> {
    get_by_id::<entity::symbols::Entity>(id).await
}

#[server]
pub async fn symbols_list(
    #[server(default)] parents: HashMap<String, Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Symbols>, ServerFnError> {
    get_all::<entity::symbols::Entity>(query_params, parents).await
}

#[server]
pub async fn symbols_list_names(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<HashSet<String>, ServerFnError> {
    get_all_names::<entity::symbols::Entity>(parents).await
}

#[server]
pub async fn symbols_add(symbols: Symbols) -> Result<(), ServerFnError> {
    add::<entity::symbols::Entity>(symbols).await
}

#[server]
pub async fn symbols_update(symbols: Symbols) -> Result<(), ServerFnError> {
    update::<entity::symbols::Entity>(symbols).await
}

#[server]
pub async fn symbols_remove(id: Uuid) -> Result<(), ServerFnError> {
    delete_by_id::<entity::symbols::Entity>(id).await
}

#[server]
pub async fn symbols_count(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<usize, ServerFnError> {
    count::<entity::symbols::Entity>(parents).await
}
