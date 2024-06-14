use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use leptos::*;
use tracing::error;
use uuid::Uuid;

use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::QueryParams;
use crate::data_providers::version::{
    version_add, version_count, version_get, version_list, version_list_names, version_remove,
    version_update, Version, VersionRow, VersionTableDataProvider,
};

use super::dataform::DataFormTrait;

pub struct VersionTable;

impl DataFormTrait for VersionTable {
    type TableDataProvider = VersionTableDataProvider;
    type RowType = VersionRow;
    type DataType = Version;

    fn new_provider(parents: HashMap<String, Uuid>) -> VersionTableDataProvider {
        VersionTableDataProvider::new(parents)
    }

    fn get_data_type_name() -> String {
        "version".to_string()
    }

    fn get_related() -> Vec<super::dataform::Related> {
        vec![super::dataform::Related {
            name: "Symbols".to_string(),
            url: "/admin/symbols?version=".to_string(),
        }]
    }
    fn get_foreign() -> Vec<super::dataform::Foreign> {
        vec![super::dataform::Foreign {
            id_name: "product_id".to_string(),
            query: "product".to_string(),
        }]
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, parents: HashMap<String, Uuid>) {
        create_effect(move |_| {
            let product_id = parents.get("product_id").cloned();
            spawn_local(async move {
                match version_list_names(product_id).await {
                    Ok(fetched_names) => {
                        fields.update(|field| {
                            field
                                .entry("Name".to_string())
                                .or_default()
                                .disallowed
                                .set(fetched_names);
                        });
                    }
                    Err(e) => tracing::error!("Failed to fetch version names: {:?}", e),
                }
            });
        });
    }

    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, version: Version) {
        fields.update(|field| {
            field
                .entry("Name".to_string())
                .or_default()
                .value
                .set(version.name);
        });
        fields.update(|field| {
            field
                .entry("Tag".to_string())
                .or_default()
                .value
                .set(version.tag);
        });
        fields.update(|field| {
            field
                .entry("Hash".to_string())
                .or_default()
                .value
                .set(version.hash);
        });
    }

    fn update_data(
        version: &mut Version,
        fields: RwSignal<IndexMap<String, Field>>,
        parents: HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();

        version.name = fields.get().get("Name").unwrap().value.get();
        version.tag = fields.get().get("Tag").unwrap().value.get();
        version.hash = fields.get().get("Hash").unwrap().value.get();
        match product_id {
            None => error!("Product ID is missing"),
            Some(product_id) => {
                version.product_id = product_id;
            }
        }
        if version.id.is_nil() {
            version.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<Version, ServerFnError<String>> {
        version_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Version>, ServerFnError<String>> {
        let product_id = parents.get("product_id").cloned();
        version_list(product_id, query_params).await
    }
    async fn list_names(
        parents: HashMap<String, Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>> {
        let product_id = parents.get("product_id").cloned();
        version_list_names(product_id).await
    }
    async fn add(data: Version) -> Result<(), ServerFnError<String>> {
        version_add(data).await
    }
    async fn update(data: Version) -> Result<(), ServerFnError<String>> {
        version_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>> {
        version_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError<String>> {
        let product_id = parents.get("product_id").cloned();
        version_count(product_id).await
    }
}

#[allow(non_snake_case)]
#[component]
pub fn VersionsPage() -> impl IntoView {
    view! {
        <DataFormPage<VersionTable>/>
    }
}
