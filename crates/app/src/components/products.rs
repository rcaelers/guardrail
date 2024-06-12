use std::collections::HashSet;

use indexmap::IndexMap;
use leptos::*;
use leptos_router::*;
use uuid::Uuid;

use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::{
    product_add, product_count, product_get, product_list, product_list_names, product_remove,
    product_update, Product, QueryParams,
};
use crate::data_providers::product::{ProductRow, ProductTableDataProvider};

use super::dataform::{DataFormTrait, ParamsTrait};

#[derive(Params, PartialEq, Clone, Debug)]
pub struct ProductParams {
    product_id: String,
}

impl ParamsTrait for ProductParams {
    fn get_id(self) -> String {
        self.product_id
    }

    fn get_param_name() -> String {
        "Product".to_string()
    }
}

pub struct ProductTable;

impl DataFormTrait for ProductTable {
    type RequestParams = ProductParams;
    type TableDataProvider = ProductTableDataProvider;
    type RowType = ProductRow;
    type DataType = Product;

    fn new_provider(_product_id: Option<Uuid>) -> ProductTableDataProvider {
        ProductTableDataProvider::new()
    }

    fn get_data_type_name() -> String {
        "product".to_string()
    }

    fn get_related_url(parent_id: Uuid) -> String {
        format!("/admin/versions/{}", parent_id)
    }

    fn get_related_name() -> Option<String>
    {
        Some("Versions".to_string())
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, _parent_id: Option<uuid::Uuid>) {
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

    fn update_data(product: &mut Product, fields: RwSignal<IndexMap<String, Field>>) {
        product.name = fields.get().get("Name").unwrap().value.get();
    }

    async fn get(id: Uuid) -> Result<Product, ServerFnError<String>> {
        product_get(id).await
    }
    async fn list(
        _parent_id: Option<Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Product>, ServerFnError<String>> {
        product_list(query_params).await
    }
    async fn list_names(
        _parent_id: Option<Uuid>,
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
    async fn count(_parent_id: Option<Uuid>) -> Result<usize, ServerFnError<String>> {
        product_count().await
    }
}

#[allow(non_snake_case)]
#[component]
pub fn ProductsPage() -> impl IntoView {
    view! {
        <DataFormPage<ProductTable>/>
    }
}
