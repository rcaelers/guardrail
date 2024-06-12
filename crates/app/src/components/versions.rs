use std::collections::HashSet;

use indexmap::IndexMap;
use leptos::*;
use leptos_router::*;
use uuid::Uuid;

use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::{
    version_add, version_count, version_get, version_list, version_list_names, version_remove,
    version_update, QueryParams, Version,
};
use crate::data_providers::version::{VersionRow, VersionTableDataProvider};

use super::dataform::{DataFormTrait, ParamsTrait};

#[derive(Params, PartialEq, Clone, Debug)]
pub struct VersionParams {
    product_id: String,
}

impl ParamsTrait for VersionParams {
    fn get_id(self) -> String {
        self.product_id
    }

    fn get_param_name() -> String {
        "Product".to_string()
    }
}

pub struct VersionTable;

impl DataFormTrait for VersionTable {
    type RequestParams = VersionParams;
    type TableDataProvider = VersionTableDataProvider;
    type RowType = VersionRow;
    type DataType = Version;

    fn new_provider(product_id: Option<Uuid>) -> VersionTableDataProvider {
        VersionTableDataProvider::new(product_id)
    }

    fn get_data_type_name() -> String {
        "version".to_string()
    }

    fn get_related_url(_parent_id: Uuid) -> String {
        "".to_string()
    }

    fn get_related_name() -> Option<String> {
        None
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, product_id: Option<uuid::Uuid>) {
        create_effect(move |_| {
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

    fn update_data(version: &mut Version, fields: RwSignal<IndexMap<String, Field>>) {
        version.name = fields.get().get("Name").unwrap().value.get();
        version.tag = fields.get().get("Tag").unwrap().value.get();
        version.hash = fields.get().get("Hash").unwrap().value.get();
    }

    async fn get(id: Uuid) -> Result<Version, ServerFnError<String>> {
        version_get(id).await
    }
    async fn list(
        parent_id: Option<Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Version>, ServerFnError<String>> {
        version_list(parent_id, query_params).await
    }
    async fn list_names(parent_id: Option<Uuid>) -> Result<HashSet<String>, ServerFnError<String>> {
        version_list_names(parent_id).await
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
    async fn count(parent_id: Option<Uuid>) -> Result<usize, ServerFnError<String>> {
        version_count(parent_id).await
    }
}

#[allow(non_snake_case)]
#[component]
pub fn VersionsPage() -> impl IntoView {
    view! {
        <DataFormPage<VersionTable>/>
    }
}
