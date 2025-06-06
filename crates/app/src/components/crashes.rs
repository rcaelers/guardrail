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
use crate::data_providers::crash::{
    // crash_add, crash_count, crash_get, crash_list, crash_list_names, crash_remove, crash_update,
    Crash,
    CrashRow,
};
use crate::table_data_provider_impl;

#[derive(Debug, Clone)]
pub struct CrashesTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl CrashesTable {
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
impl DataTableTrait for CrashesTable {
    type RowType = CrashRow;
    type DataType = Crash;

    fn new_provider(parents: HashMap<String, Uuid>) -> CrashesTable {
        CrashesTable::new(parents)
    }

    async fn capabilities(&self) -> BitFlags<Capabilities, u8> {
        Capabilities::CanDelete.into()
    }

    fn get_data_type_name() -> String {
        "crash".to_string()
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

    fn get_related() -> Vec<super::datatable::Related> {
        vec![super::datatable::Related {
            name: "Crash".to_string(),
            url: "/admin/crash?crash=".to_string(),
        }]
    }

    fn init_fields(_fields: RwSignal<Fields>, _parents: &HashMap<String, Uuid>) {}

    async fn update_fields(
        fields: RwSignal<Fields>,
        crash: Crash,
        _parents: &HashMap<String, Uuid>,
    ) {
        fields.update(|field| {
            field.insert(
                "Summary".to_string(),
                Field::new(FieldString::new(crash.summary.clone(), HashSet::new())),
            );
        });
    }

    fn update_data(crash: &mut Crash, fields: RwSignal<Fields>, parents: &HashMap<String, Uuid>) {
        let product_id = parents.get("product_id").cloned();
        let version_id = parents.get("version_id").cloned();

        let summary = fields.get().get::<FieldString>("Summary");

        crash.summary = summary.value.get();
        match product_id {
            None => error!("Product ID is missing"),
            Some(product_id) => {
                crash.product_id = product_id;
            }
        }
        match version_id {
            None => error!("Version ID is missing"),
            Some(version_id) => {
                crash.version_id = version_id;
            }
        }
        if crash.id.is_nil() {
            crash.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<Crash, ServerFnError> {
        crash_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Crash>, ServerFnError> {
        crash_list(parents, query_params).await
    }
    async fn list_names(parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        crash_list_names(parents).await
    }
    async fn add(data: Crash) -> Result<(), ServerFnError> {
        crash_add(data).await
    }
    async fn update(data: Crash) -> Result<(), ServerFnError> {
        crash_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        crash_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError> {
        crash_count(parents).await
    }
}

table_data_provider_impl!(CrashesTable);

#[allow(non_snake_case)]
#[component]
pub fn CrashesPage() -> impl IntoView {
    view! { <DataTable<CrashesTable> /> }
}
