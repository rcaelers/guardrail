use ::chrono::NaiveDateTime;
use async_trait::async_trait;
use enumflags2::BitFlags;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_struct_table::*;
use repos::product::Product;
use repos::{QueryParams, SortOrder};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use tracing::info;
use uuid::Uuid;

use super::datatable::{Capabilities, DataTableTrait, ExtraRowTrait};
use super::datatable_form::{FieldString, Fields};
use crate::classes::ClassesPreset;
use crate::components::datatable::DataTable;
use crate::components::datatable_form::Field;
use crate::data::product::{
    products_add, products_count, products_get, products_list, products_list_names,
    products_remove, products_update,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::{authenticated_user_is_admin, table_data_provider_impl};

#[derive(Debug, Clone)]
pub struct ProductTable {
    sort: VecDeque<(String, SortOrder)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

impl From<Product> for ProductRow {
    fn from(product: Product) -> Self {
        Self {
            id: product.id,
            name: product.name,
            created_at: product.created_at,
            updated_at: product.updated_at,
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

impl ProductTable {
    pub fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            filter: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}

#[async_trait]
impl DataTableTrait for ProductTable {
    type RowType = ProductRow;
    type DataType = Product;

    fn new_provider(parents: HashMap<String, Uuid>) -> ProductTable {
        ProductTable::new(parents)
    }

    fn get_data_type_name() -> String {
        "product".to_string()
    }

    async fn capabilities(&self) -> BitFlags<Capabilities, u8> {
        info!("capabilities");
        let mut cap = Capabilities::CanEdit | Capabilities::CanDelete;
        let is_admin = authenticated_user_is_admin().await;
        info!("capabilities2");
        if is_admin.unwrap_or(false) {
            cap |= Capabilities::CanAdd;
        }
        info!("capabilities3");
        cap
    }

    fn get_related() -> Vec<super::datatable::Related> {
        vec![
            super::datatable::Related {
                name: "Versions".to_string(),
                url: "/admin/versions?product=".to_string(),
            },
            super::datatable::Related {
                name: "Symbols".to_string(),
                url: "/admin/symbols?product=".to_string(),
            },
            super::datatable::Related {
                name: "Crashes".to_string(),
                url: "/admin/crashes?product=".to_string(),
            },
        ]
    }

    fn get_columns() -> Vec<String> {
        ["id", "name", "description", "created_at", "updated_at"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn init_fields(fields: RwSignal<Fields>, _parents: &HashMap<String, Uuid>) {
        fields.update(|field| {
            field.insert("Name".to_string(), Field::new(FieldString::default()));
        });
    }

    async fn update_fields(
        fields: RwSignal<Fields>,
        product: Product,
        _parents: &HashMap<String, Uuid>,
    ) {
        let name_field = fields.get_untracked().get::<FieldString>("Name");

        name_field.value.set(product.name);

        spawn_local(async move {
            match products_list_names().await {
                Ok(fetched_names) => {
                    name_field.disallowed.set(fetched_names);
                }
                Err(e) => {
                    tracing::error!("Failed to fetch product names: {:?}", e);
                }
            }
        });
    }

    fn update_data(
        product: &mut Product,
        fields: RwSignal<Fields>,
        _parents: &HashMap<String, Uuid>,
    ) {
        let name = fields.get().get::<FieldString>("Name");

        product.name = name.value.get();
        if product.id.is_nil() {
            product.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<Product, ServerFnError> {
        products_get(id).await
    }
    async fn list(
        _parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Product>, ServerFnError> {
        products_list(query_params).await
    }
    async fn list_names(_parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        products_list_names().await
    }
    async fn add(data: Product) -> Result<(), ServerFnError> {
        products_add(data.into()).await
    }
    async fn update(data: Product) -> Result<(), ServerFnError> {
        products_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        products_remove(id).await
    }
    async fn count(_parents: HashMap<String, Uuid>) -> Result<i64, ServerFnError> {
        products_count().await
    }
}

table_data_provider_impl!(ProductTable);

#[allow(non_snake_case)]
#[component]
pub fn ProductsPage() -> impl IntoView {
    view! { <DataTable<ProductTable> /> }
}
