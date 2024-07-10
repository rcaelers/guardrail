use async_trait::async_trait;
use enumflags2::BitFlags;
use indexmap::IndexMap;
use leptos::*;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use tracing::{error, info};
use uuid::Uuid;
use web_sys::console::info;

use super::datatable::{Capabilities, DataTableTrait};
use crate::components::datatable::DataTable;
use crate::components::datatable_form::Field;
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

    fn init_fields(fields: RwSignal<IndexMap<String, Field>>, parents: &HashMap<String, Uuid>) {
        fields.update(|field| {
            field
                .entry("Product".to_string())
                .or_default()
                .value
                .set("".to_string().into());
        });
        let parents = parents.clone();
        let product_signal = fields.get_untracked().get("Product").unwrap().clone();
        create_effect(move |_| {
            let parents = parents.clone();
            let product_name = product_signal.value.get();
            spawn_local(async move {
                let product = product_get_by_name(product_name.as_string()).await;

                if let Ok(product) = product {
                    let mut parents = parents.clone();
                    parents.insert("product_id".to_string(), product.id);

                    match version_list_names(parents).await {
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
                }
            });
        });
    }

    async fn update_fields(
        fields: RwSignal<IndexMap<String, Field>>,
        version: Version,
        parents: &HashMap<String, Uuid>,
    ) {
        info!("Updating fields for version {:?}", version);

        fields.update(|field| {
            field
                .entry("Product".to_string())
                .or_default()
                .value
                .set(version.product.into());
        });
        fields.update(|field| {
            field
                .entry("Name".to_string())
                .or_default()
                .value
                .set(version.name.into());
        });
        fields.update(|field| {
            field
                .entry("Tag".to_string())
                .or_default()
                .value
                .set(version.tag.into());
        });
        fields.update(|field| {
            field
                .entry("Hash".to_string())
                .or_default()
                .value
                .set(version.hash.into());
        });

        if version.product_id.is_nil() {
            if let Some(product_id) = parents.get("product_id") {
                match product_get(*product_id).await {
                    Ok(product) => fields.update(|field| {
                        field
                            .entry("Product".to_string())
                            .or_default()
                            .value
                            .set(product.name.into());
                    }),
                    Err(e) => {
                        error!("Failed to fetch product: {:?}", e);
                    }
                }
            }
        }

        let have_product = !version.product_id.is_nil() || parents.contains_key("product_id");
        info!("Have product: {}", have_product);
        fields.update(|field| {
            field
                .entry("Product".to_string())
                .or_default()
                .readonly
                .set(have_product);
        });

        if !have_product {
            match product_list_names().await {
                Ok(fetched_names) => {
                    fields.update(|field| {
                        field
                            .entry("Product".to_string())
                            .or_default()
                            .multiselect
                            .set(
                                itertools::sorted(fetched_names.iter().cloned())
                                    .collect::<Vec<_>>(),
                            );
                    });
                    fields.update(|field| {
                        field.entry("Product".to_string()).or_default().value.set(
                            itertools::sorted(fetched_names.iter().cloned())
                                .collect::<Vec<_>>()
                                .first()
                                .unwrap()
                                .clone()
                                .into(),
                        );
                    });
                }
                Err(e) => tracing::error!("Failed to fetch product names: {:?}", e),
            }
        }
    }

    fn update_data(
        version: &mut Version,
        fields: RwSignal<IndexMap<String, Field>>,
        parents: &HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();

        version.name = fields.get().get("Name").unwrap().value.get().as_string();
        version.tag = fields.get().get("Tag").unwrap().value.get().as_string();
        version.hash = fields.get().get("Hash").unwrap().value.get().as_string();
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
