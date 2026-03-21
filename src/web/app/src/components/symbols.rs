use async_trait::async_trait;
use enumflags2::BitFlags;
use leptos::prelude::*;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use tracing::error;
use uuid::Uuid;

use super::datatable::{Capabilities, DataTableTrait};
use super::datatable_form::{FieldString, Fields};
use crate::components::datatable::DataTable;
use crate::components::datatable_form::Field;
use crate::data::QueryParams;
use crate::data_providers::ExtraTableDataProvider;
use crate::data_providers::symbols::{
    Symbols, SymbolsRow, symbols_add, symbols_count, symbols_get, symbols_list, symbols_list_names,
    symbols_remove, symbols_update,
};
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

#[async_trait]
impl DataTableTrait for SymbolsTable {
    type RowType = SymbolsRow;
    type DataType = Symbols;

    fn new_provider(parents: HashMap<String, Uuid>) -> SymbolsTable {
        SymbolsTable::new(parents)
    }

    async fn capabilities(&self) -> BitFlags<Capabilities, u8> {
        Capabilities::CanDelete.into()
    }

    fn get_data_type_name() -> String {
        "symbols".to_string()
    }

    fn get_foreign() -> Vec<super::datatable::Foreign> {
        vec![
            super::datatable::Foreign {
                id_name: "product_id".to_string(),
                query: "product".to_string(),
            },
            super::datatable::Foreign {
                id_name: "version_id".to_string(),
                query: "version".to_string(),
            },
        ]
    }

    fn init_fields(_fields: RwSignal<Fields>, _parents: &HashMap<String, Uuid>) {}

    async fn update_fields(
        fields: RwSignal<Fields>,
        symbols: Symbols,
        _parents: &HashMap<String, Uuid>,
    ) {
        fields.update(|field| {
            field
                .insert("OS".to_string(), Field::new(FieldString::new(symbols.os, HashSet::new())));
        });
        fields.update(|field| {
            field.insert(
                "Arch".to_string(),
                Field::new(FieldString::new(symbols.arch, HashSet::new())),
            );
        });
        fields.update(|field| {
            field.insert(
                "BuildId".to_string(),
                Field::new(FieldString::new(symbols.build_id, HashSet::new())),
            );
        });
        fields.update(|field| {
            field.insert(
                "ModuleId".to_string(),
                Field::new(FieldString::new(symbols.module_id, HashSet::new())),
            );
        });
        fields.update(|field| {
            field.insert(
                "FileLocation".to_string(),
                Field::new(FieldString::new(symbols.storage_path, HashSet::new())),
            );
        });
    }

    fn update_data(
        symbols: &mut Symbols,
        fields: RwSignal<Fields>,
        parents: &HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();
        let version_id = parents.get("version_id").cloned();

        let os = fields.get().get::<FieldString>("OS");
        let arch = fields.get().get::<FieldString>("Arch");
        let build_id = fields.get().get::<FieldString>("BuildId");
        let module_id = fields.get().get::<FieldString>("ModuleId");
        let storage_path = fields.get().get::<FieldString>("FileLocation");

        symbols.os = os.value.get();
        symbols.arch = arch.value.get();
        symbols.build_id = build_id.value.get();
        symbols.module_id = module_id.value.get();
        symbols.storage_path = storage_path.value.get();
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

    async fn get(id: Uuid) -> Result<Symbols, ServerFnError> {
        symbols_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Symbols>, ServerFnError> {
        symbols_list(parents, query_params).await
    }
    async fn list_names(parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        symbols_list_names(parents).await
    }
    async fn add(data: Symbols) -> Result<(), ServerFnError> {
        symbols_add(data).await
    }
    async fn update(data: Symbols) -> Result<(), ServerFnError> {
        symbols_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        symbols_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError> {
        symbols_count(parents).await
    }
}

table_data_provider_impl!(SymbolsTable);

#[allow(non_snake_case)]
#[component]
pub fn SymbolsPage() -> impl IntoView {
    view! { <DataTable<SymbolsTable> /> }
}
