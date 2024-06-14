use crate::classes::ClassesPreset;
#[cfg(feature = "ssr")]
use crate::data::EntityInfo;
use crate::data::QueryParams;
#[cfg(feature = "ssr")]
use crate::data::{add, count, delete_by_id, get_all, get_all_names, get_by_id, update};
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::vec;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use sea_orm::*;

use super::{ExtraRowTrait, ExtraTableDataProvider};

#[derive(TableRow, Debug, Clone)]
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

    fn from_index(index: usize) -> Option<Self::Column> {
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

    fn extend_query(query: Select<Self>) -> Select<Self> {
        query
            .join(JoinType::LeftJoin, entity::symbols::Relation::Product.def())
            .join(JoinType::LeftJoin, entity::symbols::Relation::Version.def())
            .column_as(entity::product::Column::Name, "product")
            .column_as(entity::version::Column::Name, "version")
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
impl From<Symbols> for entity::symbols::ActiveModel {
    fn from(symbols: Symbols) -> Self {
        Self {
            id: Set(symbols.id),
            os: Set(symbols.os),
            arch: Set(symbols.arch),
            build_id: Set(symbols.build_id),
            module_id: Set(symbols.module_id),
            file_location: Set(symbols.file_location),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
            product_id: Set(symbols.product_id),
            version_id: Set(symbols.version_id),
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

#[derive(Debug, Clone)]
pub struct SymbolsTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl SymbolsTableDataProvider {
    pub fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            filter: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}

impl ExtraTableDataProvider<SymbolsRow> for SymbolsTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.filter
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl TableDataProvider<SymbolsRow> for SymbolsTableDataProvider {
    async fn get_rows(
        &self,
        range: Range<usize>,
    ) -> Result<(Vec<SymbolsRow>, Range<usize>), String> {
        let product_id = self.parents.get("product_id").cloned();
        let version_id = self.parents.get("version_id").cloned();
        let symbols = symbols_list(
            product_id,
            version_id,
            QueryParams {
                filter: self.filter.get_untracked().trim().to_string(),
                sorting: self.sort.clone(),
                range: range.clone(),
            },
        )
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|symbols| symbols.into())
        .collect::<Vec<SymbolsRow>>();

        let len = symbols.len();
        Ok((symbols, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        let product_id = self.parents.get("product_id").cloned();
        let version_id = self.parents.get("version_id").cloned();
        symbols_count(product_id, version_id).await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.filter.track();
        self.update.track();
    }
}

#[server]
pub async fn symbols_get(id: Uuid) -> Result<Symbols, ServerFnError<String>> {
    get_by_id::<Symbols, entity::symbols::Entity>(id).await
}

#[server]
pub async fn symbols_list(
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Symbols>, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::symbols::Column::ProductId, product_id));
    }
    if let Some(version_id) = version_id {
        parents.push((entity::symbols::Column::VersionId, version_id));
    }
    get_all::<Symbols, entity::symbols::Entity>(query_params, parents).await
}

#[server]
pub async fn symbols_list_names(
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
) -> Result<HashSet<String>, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::symbols::Column::ProductId, product_id));
    }
    if let Some(version_id) = version_id {
        parents.push((entity::symbols::Column::VersionId, version_id));
    }
    get_all_names::<entity::symbols::Entity>(parents).await
}

#[server]
pub async fn symbols_add(symbols: Symbols) -> Result<(), ServerFnError<String>> {
    add::<Symbols, entity::symbols::Entity>(symbols).await
}

#[server]
pub async fn symbols_update(symbols: Symbols) -> Result<(), ServerFnError<String>> {
    update::<Symbols, entity::symbols::Entity>(symbols).await
}

#[server]
pub async fn symbols_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::symbols::Entity>(id).await
}

#[server]
pub async fn symbols_count(
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
) -> Result<usize, ServerFnError<String>> {
    let mut parents = vec![];
    if let Some(product_id) = product_id {
        parents.push((entity::symbols::Column::ProductId, product_id));
    }
    if let Some(version_id) = version_id {
        parents.push((entity::symbols::Column::VersionId, version_id));
    }
    count::<entity::symbols::Entity>(parents).await
}
