use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use sea_orm::*;
    use sea_query::Expr;
    use crate::entity;
    use crate::data::{
        add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
    };
    use crate::auth::AuthenticatedUser;
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct VersionRow {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
}

#[cfg(feature = "ssr")]
#[derive(FromQueryResult, Debug, Default, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Version {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
impl EntityInfo for entity::version::Entity {
    type View = Version;

    fn filter_column() -> Self::Column {
        entity::version::Column::Name
    }

    fn index_to_column(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::version::Column::Id),
            1 => Some(entity::version::Column::Name),
            2 => Some(entity::version::Column::Hash),
            3 => Some(entity::version::Column::Tag),
            4 => Some(entity::version::Column::ProductId),
            5 => Some(entity::version::Column::CreatedAt),
            6 => Some(entity::version::Column::UpdatedAt),
            _ => None,
        }
    }

    fn extend_query_for_view(query: Select<Self>) -> Select<Self> {
        query
            .join(JoinType::LeftJoin, entity::version::Relation::Product.def())
            .column_as(entity::product::Column::Name, "product")
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
            "product_id" => Some(entity::version::Column::ProductId),
            _ => None,
        }
    }
}

impl From<Version> for VersionRow {
    fn from(version: Version) -> Self {
        Self {
            id: version.id,
            name: version.name,
            hash: version.hash,
            tag: version.tag,
            product_id: Some(version.product_id),
            created_at: version.created_at,
            updated_at: version.updated_at,
            product: version.product,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::version::Model> for Version {
    fn from(model: entity::version::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            hash: model.hash,
            tag: model.tag,
            product_id: model.product_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
            product: "".to_string(),
        }
    }
}

#[cfg(feature = "ssr")]
impl From<Version> for entity::version::ActiveModel {
    fn from(version: Version) -> Self {
        Self {
            id: Set(version.id),
            name: Set(version.name),
            hash: Set(version.hash),
            tag: Set(version.tag),
            product_id: Set(version.product_id),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
        }
    }
}

impl ExtraRowTrait for VersionRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[server]
pub async fn version_get(id: Uuid) -> Result<Version, ServerFnError> {
    get_by_id::<entity::version::Entity>(id).await
}

#[server]
pub async fn version_list(
    #[server(default)] parents: HashMap<String, Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Version>, ServerFnError> {
    get_all::<entity::version::Entity>(query_params, parents).await
}

#[server]
pub async fn version_list_names(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<HashSet<String>, ServerFnError> {
    get_all_names::<entity::version::Entity>(parents).await
}

#[server]
pub async fn version_add(version: Version) -> Result<(), ServerFnError> {
    add::<entity::version::Entity>(version).await
}

#[server]
pub async fn version_update(version: Version) -> Result<(), ServerFnError> {
    update::<entity::version::Entity>(version).await
}

#[server]
pub async fn version_remove(id: Uuid) -> Result<(), ServerFnError> {
    delete_by_id::<entity::version::Entity>(id).await
}

#[server]
pub async fn version_count(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<usize, ServerFnError> {
    count::<entity::version::Entity>(parents).await
}
