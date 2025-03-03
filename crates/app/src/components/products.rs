use async_trait::async_trait;
use enumflags2::BitFlags;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

use super::datatable::{Capabilities, DataTableTrait};
use super::datatable_form::{FieldString, Fields};
use crate::components::datatable::DataTable;
use crate::components::datatable_form::Field;
use crate::data::QueryParams;
use crate::data_providers::product::{
    product_add, product_count, product_get, product_list, product_list_names, product_remove,
    product_update, Product, ProductRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::{authenticated_user_is_admin, table_data_provider_impl};

#[derive(Debug, Clone)]
pub struct ProductTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
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
        let mut cap = Capabilities::CanEdit | Capabilities::CanDelete;
        if authenticated_user_is_admin().await.unwrap_or(false) {
            cap |= Capabilities::CanAdd;
        }
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
            match product_list_names().await {
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
        product_get(id).await
    }
    async fn list(
        _parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Product>, ServerFnError> {
        product_list(query_params).await
    }
    async fn list_names(_parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        product_list_names().await
    }
    async fn add(data: Product) -> Result<(), ServerFnError> {
        product_add(data).await
    }
    async fn update(data: Product) -> Result<(), ServerFnError> {
        product_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        product_remove(id).await
    }
    async fn count(_parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError> {
        product_count().await
    }
}

table_data_provider_impl!(ProductTable);

#[allow(non_snake_case)]
#[component]
pub fn ProductsPage() -> impl IntoView {
    view! { <DataTable<ProductTable> /> }
}
