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
use crate::data_providers::crash::{
    crash_add, crash_count, crash_get, crash_list, crash_list_names, crash_remove, crash_update,
    Crash, CrashRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::table_data_provider_impl;

#[derive(Debug, Clone)]
pub struct CrashTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl CrashTable {
    pub fn new(parents: HashMap<String, Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            filter: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            parents,
        }
    }
}
impl DataFormTrait for CrashTable {
    type TableDataProvider = CrashTable;
    type RowType = CrashRow;
    type DataType = Crash;

    fn new_provider(parents: HashMap<String, Uuid>) -> CrashTable {
        CrashTable::new(parents)
    }

    fn capabilities() -> BitFlags<Capabilities, u8> {
        Capabilities::CanDelete.into()
    }

    fn get_data_type_name() -> String {
        "crash".to_string()
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

    fn initial_fields(_fields: RwSignal<IndexMap<String, Field>>, _parents: HashMap<String, Uuid>) {}

    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, crash: Crash) {
        fields.update(|field| {
            field
                .entry("Summary".to_string())
                .or_default()
                .value
                .set(crash.summary);
        });
    }

    fn update_data(
        crash: &mut Crash,
        fields: RwSignal<IndexMap<String, Field>>,
        parents: HashMap<String, Uuid>,
    ) {
        let product_id = parents.get("product_id").cloned();
        let version_id = parents.get("version_id").cloned();

        crash.summary = fields.get().get("Summary").unwrap().value.get();
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

    async fn get(id: Uuid) -> Result<Crash, ServerFnError<String>> {
        crash_get(id).await
    }
    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Crash>, ServerFnError<String>> {
        crash_list(parents, query_params).await
    }
    async fn list_names(
        parents: HashMap<String, Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>> {
        crash_list_names(parents).await
    }
    async fn add(data: Crash) -> Result<(), ServerFnError<String>> {
        crash_add(data).await
    }
    async fn update(data: Crash) -> Result<(), ServerFnError<String>> {
        crash_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>> {
        crash_remove(id).await
    }
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError<String>> {
        crash_count(parents).await
    }
}

table_data_provider_impl!(CrashTable);

#[allow(non_snake_case)]
#[component]
pub fn CrashPage() -> impl IntoView {
    view! {
        <DataFormPage<CrashTable>/>
    }
}
