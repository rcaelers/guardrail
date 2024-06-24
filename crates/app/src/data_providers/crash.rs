use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::vec;
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use sea_orm::*;
    use sea_query::Expr;
    use crate::entity;
    use crate::auth::AuthenticatedUser;
    use crate::data::{
        add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
    };
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct CrashRow {
    pub id: Uuid,
    pub product: String,
    pub version: String,
    pub summary: String,
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
pub struct Crash {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub summary: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Crash {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub summary: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
}

#[cfg(feature = "ssr")]
impl EntityInfo for entity::crash::Entity {
    type View = Crash;

    fn filter_column() -> Self::Column {
        entity::crash::Column::Report
    }

    fn index_to_column(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::crash::Column::Id),
            1 => Some(entity::crash::Column::Report),
            2 => Some(entity::crash::Column::Summary),
            3 => Some(entity::crash::Column::CreatedAt),
            4 => Some(entity::crash::Column::UpdatedAt),
            _ => None,
        }
    }

    fn extend_query_for_view(query: Select<Self>) -> Select<Self> {
        query
            .join(JoinType::LeftJoin, entity::crash::Relation::Product.def())
            .join(JoinType::LeftJoin, entity::crash::Relation::Version.def())
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
            "product_id" => Some(entity::crash::Column::ProductId),
            "version_id" => Some(entity::crash::Column::VersionId),
            _ => None,
        }
    }
}
impl From<Crash> for CrashRow {
    fn from(crash: Crash) -> Self {
        Self {
            id: crash.id,
            summary: crash.summary,
            created_at: crash.created_at,
            updated_at: crash.updated_at,
            product_id: Some(crash.product_id),
            version_id: Some(crash.version_id),
            product: crash.product,
            version: crash.version,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::crash::Model> for Crash {
    fn from(model: entity::crash::Model) -> Self {
        Self {
            id: model.id,
            summary: model.summary,
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
impl From<Crash> for entity::crash::ActiveModel {
    fn from(crash: Crash) -> Self {
        Self {
            id: Set(crash.id),
            report: sea_orm::NotSet,
            summary: Set(crash.summary),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
            product_id: Set(crash.product_id),
            version_id: Set(crash.version_id),
        }
    }
}

impl ExtraRowTrait for CrashRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.summary.clone()
    }
}

#[server]
pub async fn crash_get(id: Uuid) -> Result<Crash, ServerFnError> {
    get_by_id::<entity::crash::Entity>(id).await
}

#[server]
pub async fn crash_list(
    #[server(default)] parents: HashMap<String, Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Crash>, ServerFnError> {
    get_all::<entity::crash::Entity>(query_params, parents).await
}

#[server]
pub async fn crash_list_names(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<HashSet<String>, ServerFnError> {
    get_all_names::<entity::crash::Entity>(parents).await
}

#[server]
pub async fn crash_add(crash: Crash) -> Result<(), ServerFnError> {
    add::<entity::crash::Entity>(crash).await
}

#[server]
pub async fn crash_update(crash: Crash) -> Result<(), ServerFnError> {
    update::<entity::crash::Entity>(crash).await
}

#[server]
pub async fn crash_remove(id: Uuid) -> Result<(), ServerFnError> {
    delete_by_id::<entity::crash::Entity>(id).await
}

#[server]
pub async fn crash_count(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<usize, ServerFnError> {
    count::<entity::crash::Entity>(parents).await
}
