use indexmap::IndexMap;
use leptos::*;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

use super::dataform::DataFormTrait;
use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::QueryParams;
use crate::data_providers::product::{
    product_add, product_count, product_get, product_list, product_list_names, product_remove,
    product_update, Product, ProductRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::table_data_provider_impl;

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

impl DataFormTrait for ProductTable {
    type TableDataProvider = ProductTable;
    type RowType = ProductRow;
    type DataType = Product;

    fn new_provider(parents: HashMap<String, Uuid>) -> ProductTable {
        ProductTable::new(parents)
    }

    fn get_data_type_name() -> String {
        "product".to_string()
    }

    fn get_related() -> Vec<super::dataform::Related> {
        vec![
            super::dataform::Related {
                name: "Versions".to_string(),
                url: "/admin/versions?product=".to_string(),
            },
            super::dataform::Related {
                name: "Symbols".to_string(),
                url: "/admin/symbols?product=".to_string(),
            },
            super::dataform::Related {
                name: "Crashes".to_string(),
                url: "/admin/crashes?product=".to_string(),
            },
        ]
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, _parents: HashMap<String, Uuid>) {
        create_effect(move |_| {
            spawn_local(async move {
                match product_list_names().await {
                    Ok(fetched_names) => {
                        fields.update(|field| {
                            field
                                .entry("Name".to_string())
                                .or_default()
                                .disallowed
                                .set(fetched_names);
                        });
                    }
                    Err(e) => tracing::error!("Failed to fetch product names: {:?}", e),
                }
            });
        });
    }

    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, product: Product) {
        fields.update(|field| {
            field
                .entry("Name".to_string())
                .or_default()
                .value
                .set(product.name);
        });
    }

    fn update_data(
        product: &mut Product,
        fields: RwSignal<IndexMap<String, Field>>,
        _parents: HashMap<String, Uuid>,
    ) {
        product.name = fields.get().get("Name").unwrap().value.get();
        if product.id.is_nil() {
            product.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<Product, ServerFnError<String>> {
        product_get(id).await
    }
    async fn list(
        _parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Product>, ServerFnError<String>> {
        product_list(query_params).await
    }
    async fn list_names(
        _parents: HashMap<String, Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>> {
        product_list_names().await
    }
    async fn add(data: Product) -> Result<(), ServerFnError<String>> {
        product_add(data).await
    }
    async fn update(data: Product) -> Result<(), ServerFnError<String>> {
        product_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>> {
        product_remove(id).await
    }
    async fn count(_parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError<String>> {
        product_count().await
    }
}

table_data_provider_impl!(ProductTable);

#[allow(non_snake_case)]
#[component]
pub fn ProductsPage() -> impl IntoView {
    view! {
        <DataFormPage<ProductTable>/>
    }
}
