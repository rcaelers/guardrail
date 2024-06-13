use crate::classes::ClassesPreset;
use crate::data::QueryParams;
#[cfg(feature = "ssr")]
use crate::data::{
    add, count, delete_by_id, get_all, get_all_names, get_by_id, update, ColumnInfo,
};
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use sea_orm::*;

use super::{ExtraRowTrait, ExtraTableDataProvider};

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
impl ColumnInfo for entity::product::Column {
    fn name_column() -> Self {
        entity::product::Column::Name
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(entity::product::Column::Id),
            1 => Some(entity::product::Column::Name),
            2 => Some(entity::product::Column::CreatedAt),
            3 => Some(entity::product::Column::UpdatedAt),
            _ => None,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::product::Model> for Product {
    fn from(model: entity::product::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<Product> for entity::product::ActiveModel {
    fn from(product: Product) -> Self {
        Self {
            id: Set(product.id),
            name: Set(product.name),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
        }
    }
}

impl ExtraRowTrait for ProductRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ProductTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
}

impl ExtraTableDataProvider<ProductRow> for ProductTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.name
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl ProductTableDataProvider {
    pub fn new() -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
        }
    }
}

impl Default for ProductTableDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TableDataProvider<ProductRow> for ProductTableDataProvider {
    async fn get_rows(
        &self,
        range: Range<usize>,
    ) -> Result<(Vec<ProductRow>, Range<usize>), String> {
        let products = product_list(QueryParams {
            name: self.name.get_untracked().trim().to_string(),
            sorting: self.sort.clone(),
            range: range.clone(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|product| ProductRow {
            id: product.id,
            created_at: product.created_at,
            updated_at: product.updated_at,
            name: product.name.clone(),
        })
        .collect::<Vec<ProductRow>>();

        let len = products.len();
        Ok((products, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        product_count().await.ok()
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
pub async fn product_get(id: Uuid) -> Result<Product, ServerFnError<String>> {
    get_by_id::<Product, entity::product::Entity>(id).await
}

#[server]
pub async fn product_list(query: QueryParams) -> Result<Vec<Product>, ServerFnError<String>> {
    get_all::<Product, entity::product::Entity>(query).await
}

#[server]
pub async fn product_list_names() -> Result<HashSet<String>, ServerFnError<String>> {
    get_all_names::<entity::product::Entity>().await
}

#[server]
pub async fn product_add(product: Product) -> Result<(), ServerFnError<String>> {
    add::<Product, entity::product::Entity>(product).await
}

#[server]
pub async fn product_update(product: Product) -> Result<(), ServerFnError<String>> {
    update::<Product, entity::product::Entity>(product).await
}

#[server]
pub async fn product_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::product::Entity>(id).await
}

#[server]
pub async fn product_count() -> Result<usize, ServerFnError<String>> {
    count::<entity::product::Entity>().await
}
