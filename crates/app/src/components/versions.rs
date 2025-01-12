use async_trait::async_trait;
use enumflags2::BitFlags;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use tracing::error;
use uuid::Uuid;

use super::datatable::{Capabilities, DataTableTrait};
use super::datatable_form::Fields;
use crate::components::datatable::DataTable;
use crate::components::datatable_form::{Field, FieldCombo, FieldString};
use crate::data::QueryParams;
use crate::data_providers::product::{product_get, product_get_by_name, product_list_names};
use crate::data_providers::version::{
    version_add, version_count, version_get, version_list, version_list_names, version_remove,
    version_update, Version, VersionRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::table_data_provider_impl;

#[derive(Debug, Clone)]
pub struct VersionTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl VersionTable {
    fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            filter: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}

#[async_trait]
impl DataTableTrait for VersionTable {
    type RowType = VersionRow;
    type DataType = Version;

    fn new_provider(parents: HashMap<String, Uuid>) -> Self {
        VersionTable::new(parents)
    }

    async fn capabilities(&self) -> BitFlags<Capabilities, u8> {
        Capabilities::CanEdit | Capabilities::CanDelete | Capabilities::CanAdd
    }

    fn get_data_type_name() -> String {
        "version".to_string()
    }

    fn get_related() -> Vec<super::datatable::Related> {
        vec![
            super::datatable::Related {
                name: "Symbols".to_string(),
                url: "/admin/symbols?version=".to_string(),
            },
            super::datatable::Related {
                name: "Crashes".to_string(),
                url: "/admin/crashes?version=".to_string(),
            },
        ]
    }
    fn get_foreign() -> Vec<super::datatable::Foreign> {
        vec![super::datatable::Foreign {
            id_name: "product_id".to_string(),
            query: "product".to_string(),
        }]
    }

    fn init_fields(fields: RwSignal<Fields>, parents: &HashMap<String, Uuid>) {
        fields.update(|field| {
            field.insert("Product".to_string(), Field::new(FieldCombo::default()));
        });
        fields.update(|field| {
            field.insert("Name".to_string(), Field::new(FieldString::default()));
        });
        let parents = parents.clone();
        let product_field = fields.get_untracked().get::<FieldCombo>("Product");
        let name_field = fields.get_untracked().get::<FieldString>("Name");

        Effect::new(move |_| {
            let parents = parents.clone();
            let product_name = product_field.value.get();
            spawn_local(async move {
                let product = product_get_by_name(product_name).await;

                if let Ok(product) = product {
                    let mut parents = parents.clone();
                    parents.insert("product_id".to_string(), product.id);

                    match version_list_names(parents).await {
                        Ok(fetched_names) => {
                            name_field.disallowed.set(fetched_names);
                        }
                        Err(e) => tracing::error!("Failed to fetch version names: {:?}", e),
                    }
                }
            });
        });
    }

    async fn update_fields(
        fields: RwSignal<Fields>,
        version: Version,
        parents: &HashMap<String, Uuid>,
    ) {
        let product_field = fields.get_untracked().get::<FieldCombo>("Product");
        let name_field = fields.get_untracked().get::<FieldString>("Name");
        let product_options = fields.get_untracked().get_options("Product");

        product_field.value.set(version.product);
        name_field.value.set(version.name);

        fields.update(|field| {
            field.insert(
                "Tag".to_string(),
                Field::new(FieldString::new(version.tag, HashSet::new())),
            );
        });
        fields.update(|field| {
            field.insert(
                "Hash".to_string(),
                Field::new(FieldString::new(version.hash, HashSet::new())),
            );
        });

        if version.product_id.is_nil() {
            if let Some(product_id) = parents.get("product_id") {
                match product_get(*product_id).await {
                    Ok(product) => product_field.value.set(product.name),
                    Err(e) => {
                        error!("Failed to fetch product: {:?}", e);
                    }
                }
            }
        }

        let have_product = !version.product_id.is_nil() || parents.contains_key("product_id");
        product_options.readonly.set(have_product);

        match product_list_names().await {
            Ok(fetched_names) => {
                product_field
                    .multiselect
                    .set(itertools::sorted(fetched_names.iter().cloned()).collect::<Vec<_>>());

                if !have_product {
                    product_field.value.set(
                        itertools::sorted(fetched_names.iter().cloned())
                            .collect::<Vec<_>>()
                            .first()
                            .unwrap()
                            .clone(),
                    );
                }
            }
            Err(e) => tracing::error!("Failed to fetch product names: {:?}", e),
        }
    }

    fn update_data(
        version: &mut Version,
        fields: RwSignal<Fields>,
        parents: &HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();

        version.name = fields.get().get::<FieldString>("Name").value.get();
        version.tag = fields.get().get::<FieldString>("Tag").value.get();
        version.hash = fields.get().get::<FieldString>("Hash").value.get();
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

    async fn get(id: Uuid) -> Result<Version, ServerFnError> {
        version_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Version>, ServerFnError> {
        version_list(parents, query_params).await
    }
    async fn list_names(parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        version_list_names(parents).await
    }
    async fn add(data: Version) -> Result<(), ServerFnError> {
        version_add(data).await
    }
    async fn update(data: Version) -> Result<(), ServerFnError> {
        version_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        version_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError> {
        version_count(parents).await
    }
}

table_data_provider_impl!(VersionTable);

#[allow(non_snake_case)]
#[component]
pub fn VersionsPage() -> impl IntoView {
    view! {
        <DataTable<VersionTable>/>
    }
}
