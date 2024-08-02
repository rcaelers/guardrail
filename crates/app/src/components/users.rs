use async_trait::async_trait;
use enumflags2::BitFlags;
use leptos::*;
use leptos_struct_table::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

use super::datatable::{Capabilities, DataTableTrait};
use super::datatable_form::{FieldString, Fields};
use crate::components::datatable::DataTable;
use crate::components::datatable_form::Field;
use crate::data::QueryParams;
use crate::data_providers::user::{
    user_add, user_count, user_get, user_list, user_list_names, user_remove, user_update, User,
    UserRow,
};
use crate::data_providers::ExtraTableDataProvider;
use crate::table_data_provider_impl;

#[derive(Debug, Clone)]
pub struct UserTable {
    sort: VecDeque<(usize, ColumnSort)>,
    filter: RwSignal<String>,
    update: RwSignal<u64>,
    parents: HashMap<String, Uuid>,
}

impl UserTable {
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
impl DataTableTrait for UserTable {
    type RowType = UserRow;
    type DataType = User;

    fn new_provider(parents: HashMap<String, Uuid>) -> UserTable {
        UserTable::new(parents)
    }

    async fn capabilities(&self) -> BitFlags<Capabilities, u8> {
        Capabilities::CanEdit | Capabilities::CanAdd | Capabilities::CanDelete
    }

    fn get_data_type_name() -> String {
        "user".to_string()
    }

    fn init_fields(_fields: RwSignal<Fields>, _parents: &HashMap<String, Uuid>) {}

    async fn update_fields(fields: RwSignal<Fields>, user: User, _parents: &HashMap<String, Uuid>) {
        create_effect(move |_| {
            let user_name = user.username.clone();
            spawn_local(async move {
                match user_list_names().await {
                    Ok(fetched_names) => {
                        fields.update(|field| {
                            field.insert(
                                "Name".to_string(),
                                Field::new(FieldString::new(user_name, fetched_names)),
                            );
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch product names: {:?}", e);
                    }
                }
            });
        });
    }

    fn update_data(user: &mut User, fields: RwSignal<Fields>, _parents: &HashMap<String, Uuid>) {
        let username = fields.get().get::<FieldString>("Name");

        user.username = username.value.get();
        if user.id.is_nil() {
            user.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<User, ServerFnError> {
        user_get(id).await
    }
    async fn list(
        _parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<User>, ServerFnError> {
        user_list(query_params).await
    }
    async fn list_names(_parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError> {
        user_list_names().await
    }
    async fn add(data: User) -> Result<(), ServerFnError> {
        user_add(data).await
    }
    async fn update(data: User) -> Result<(), ServerFnError> {
        user_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError> {
        user_remove(id).await
    }
    async fn count(_parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError> {
        user_count().await
    }
}

table_data_provider_impl!(UserTable);

#[allow(non_snake_case)]
#[component]
pub fn UsersPage() -> impl IntoView {
    view! {
        <DataTable<UserTable>/>
    }
}
