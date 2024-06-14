use crate::classes::ClassesPreset;
use crate::data::QueryParams;
#[cfg(feature = "ssr")]
use crate::data::{
    add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
};
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use sea_orm::*;

use super::{ExtraRowTrait, ExtraTableDataProvider};

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

    fn from_index(index: usize) -> Option<Self::Column> {
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

    fn extend_query(query: Select<Self>) -> Select<Self> {
        query
            .join(JoinType::LeftJoin, entity::version::Relation::Product.def())
            .column_as(entity::product::Column::Name, "product")
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

#[derive(Debug, Clone)]
pub struct VersionTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl VersionTableDataProvider {
    pub fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}

impl ExtraTableDataProvider<VersionRow> for VersionTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.name
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl TableDataProvider<VersionRow> for VersionTableDataProvider {
    async fn get_rows(
        &self,
        range: Range<usize>,
    ) -> Result<(Vec<VersionRow>, Range<usize>), String> {
        let product_id = self.parents.get("product_id").cloned();

        let versions = version_list(
            product_id,
            QueryParams {
                filter: self.name.get_untracked().trim().to_string(),
                sorting: self.sort.clone(),
                range: range.clone(),
            },
        )
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|version| VersionRow {
            id: version.id,
            product_id: Some(version.product_id),
            product: version.product.clone(),
            hash: version.hash.clone(),
            tag: version.tag.clone(),
            created_at: version.created_at,
            updated_at: version.updated_at,
            name: version.name.clone(),
        })
        .collect::<Vec<VersionRow>>();

        let len = versions.len();
        Ok((versions, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        let product_id = self.parents.get("product_id").cloned();

        version_count(product_id).await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.name.track();
        self.update.track();
    }
}

#[server]
pub async fn version_get(id: Uuid) -> Result<Version, ServerFnError<String>> {
    get_by_id::<Version, entity::version::Entity>(id).await
}

#[server]
pub async fn version_list(
    product_id: Option<Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Version>, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::version::Column::ProductId, product_id));
    }
    get_all::<Version, entity::version::Entity>(query_params, parents).await
}

#[server]
pub async fn version_list_names(
    product_id: Option<Uuid>,
) -> Result<HashSet<String>, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::version::Column::ProductId, product_id));
    }
    get_all_names::<entity::version::Entity>(parents).await
}

#[server]
pub async fn version_add(version: Version) -> Result<(), ServerFnError<String>> {
    add::<Version, entity::version::Entity>(version).await
}

#[server]
pub async fn version_update(version: Version) -> Result<(), ServerFnError<String>> {
    update::<Version, entity::version::Entity>(version).await
}

#[server]
pub async fn version_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::version::Entity>(id).await
}

#[server]
pub async fn version_count(product_id: Option<Uuid>) -> Result<usize, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::version::Column::ProductId, product_id));
    }
    count::<entity::version::Entity>(parents).await
}
