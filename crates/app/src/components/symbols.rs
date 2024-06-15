use enumflags2::BitFlags;
use indexmap::IndexMap;
use leptos::*;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use tracing::error;
use uuid::Uuid;

use super::dataform::{Capabilities, DataFormTrait};
use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::QueryParams;
use crate::data_providers::symbols::{
    symbols_add, symbols_count, symbols_get, symbols_list, symbols_list_names, symbols_remove,
    symbols_update, Symbols, SymbolsRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::table_data_provider_impl;

#[derive(Debug, Clone)]
pub struct SymbolsTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl SymbolsTable {
    pub fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            filter: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}
impl DataFormTrait for SymbolsTable {
    type TableDataProvider = SymbolsTable;
    type RowType = SymbolsRow;
    type DataType = Symbols;

    fn new_provider(parents: HashMap<String, Uuid>) -> SymbolsTable {
        SymbolsTable::new(parents)
    }

    fn capabilities() -> BitFlags<Capabilities, u8> {
        Capabilities::CanDelete.into()
    }

    fn get_data_type_name() -> String {
        "symbols".to_string()
    }

    fn get_foreign() -> Vec<super::dataform::Foreign> {
        vec![
            super::dataform::Foreign {
                id_name: "product_id".to_string(),
                query: "product".to_string(),
            },
            super::dataform::Foreign {
                id_name: "version_id".to_string(),
                query: "version".to_string(),
            },
        ]
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, parents: HashMap<String, Uuid>) {
        let parents = parents.clone();
        create_effect(move |_| {
            let parents = parents.clone();
            spawn_local(async move {
                match symbols_list_names(parents).await {
                    Ok(fetched_names) => {
                        fields.update(|field| {
                            field
                                .entry("Name".to_string())
                                .or_default()
                                .disallowed
                                .set(fetched_names);
                        });
                    }
                    Err(e) => tracing::error!("Failed to fetch symbols names: {:?}", e),
                }
            });
        });
    }

    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, symbols: Symbols) {
        fields.update(|field| {
            field
                .entry("OS".to_string())
                .or_default()
                .value
                .set(symbols.os);
        });
        fields.update(|field| {
            field
                .entry("Arch".to_string())
                .or_default()
                .value
                .set(symbols.arch);
        });
        fields.update(|field| {
            field
                .entry("BuildId".to_string())
                .or_default()
                .value
                .set(symbols.build_id);
        });
        fields.update(|field| {
            field
                .entry("ModuleId".to_string())
                .or_default()
                .value
                .set(symbols.module_id);
        });
        fields.update(|field| {
            field
                .entry("FileLocation".to_string())
                .or_default()
                .value
                .set(symbols.file_location);
        });
    }

    fn update_data(
        symbols: &mut Symbols,
        fields: RwSignal<IndexMap<String, Field>>,
        parents: HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();
        let version_id = parents.get("version_id").cloned();

        symbols.os = fields.get().get("Name").unwrap().value.get();
        symbols.arch = fields.get().get("Arch").unwrap().value.get();
        symbols.build_id = fields.get().get("BuildId").unwrap().value.get();
        symbols.module_id = fields.get().get("ModuleId").unwrap().value.get();
        symbols.file_location = fields.get().get("FileLocation").unwrap().value.get();
        match product_id {
            None => error!("Product ID is missing"),
            Some(product_id) => {
                symbols.product_id = product_id;
            }
        }
        match version_id {
            None => error!("Version ID is missing"),
            Some(version_id) => {
                symbols.version_id = version_id;
            }
        }
        if symbols.id.is_nil() {
            symbols.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<Symbols, ServerFnError<String>> {
        symbols_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Symbols>, ServerFnError<String>> {
        symbols_list(parents, query_params).await
    }
    async fn list_names(
        parents: HashMap<String, Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>> {
        symbols_list_names(parents).await
    }
    async fn add(data: Symbols) -> Result<(), ServerFnError<String>> {
        symbols_add(data).await
    }
    async fn update(data: Symbols) -> Result<(), ServerFnError<String>> {
        symbols_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>> {
        symbols_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError<String>> {
        symbols_count(parents).await
    }
}

table_data_provider_impl!(SymbolsTable);

#[allow(non_snake_case)]
#[component]
pub fn SymbolsPage() -> impl IntoView {
    view! {
        <DataFormPage<SymbolsTable>/>
    }
}
