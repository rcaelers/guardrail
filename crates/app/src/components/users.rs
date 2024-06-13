use std::collections::HashSet;

use indexmap::IndexMap;
use leptos::*;
use leptos_router::*;
use uuid::Uuid;

use crate::components::dataform::DataFormPage;
use crate::components::form::Field;
use crate::data::QueryParams;
use crate::data_providers::user::{user_add, user_count, user_get, user_list, user_list_names, user_remove, user_update, User, UserRow, UserTableDataProvider};

use super::dataform::{DataFormTrait, ParamsTrait};

#[derive(Params, PartialEq, Clone, Debug)]
pub struct UserParams {
    user_id: String,
}

impl ParamsTrait for UserParams {
    fn get_id(self) -> String {
        self.user_id
    }

    fn get_param_name() -> String {
        "User".to_string()
    }
}

pub struct UserTable;

impl DataFormTrait for UserTable {
    type RequestParams = UserParams;
    type TableDataProvider = UserTableDataProvider;
    type RowType = UserRow;
    type DataType = User;

    fn new_provider(_user_id: Option<Uuid>) -> UserTableDataProvider {
        UserTableDataProvider::new()
    }

    fn get_data_type_name() -> String {
        "user".to_string()
    }

    fn get_related_url(_parent_id: Uuid) -> String {
        "".to_string()
    }

    fn get_related_name() -> Option<String> {
        None
    }

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, _parent_id: Option<uuid::Uuid>) {
        create_effect(move |_| {
            spawn_local(async move {
                match user_list_names().await {
                    Ok(fetched_names) => {
                        fields.update(|field| {
                            field
                                .entry("Name".to_string())
                                .or_default()
                                .disallowed
                                .set(fetched_names);
                        });
                    }
                    Err(e) => tracing::error!("Failed to fetch user names: {:?}", e),
                }
            });
        });
    }

    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, user: User) {
        fields.update(|field| {
            field
                .entry("Name".to_string())
                .or_default()
                .value
                .set(user.username);
        });
    }

    fn update_data(
        user: &mut User,
        fields: RwSignal<IndexMap<String, Field>>,
        _parent_id: Option<Uuid>,
    ) {
        user.username = fields.get().get("Name").unwrap().value.get();
        if user.id.is_nil() {
            user.id = Uuid::new_v4();
        }
    }

    async fn get(id: Uuid) -> Result<User, ServerFnError<String>> {
        user_get(id).await
    }
    async fn list(
        _parent_id: Option<Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<User>, ServerFnError<String>> {
        user_list(query_params).await
    }
    async fn list_names(
        _parent_id: Option<Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>> {
        user_list_names().await
    }
    async fn add(data: User) -> Result<(), ServerFnError<String>> {
        user_add(data).await
    }
    async fn update(data: User) -> Result<(), ServerFnError<String>> {
        user_update(data).await
    }
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>> {
        user_remove(id).await
    }
    async fn count(_parent_id: Option<Uuid>) -> Result<usize, ServerFnError<String>> {
        user_count().await
    }
}

#[allow(non_snake_case)]
#[component]
pub fn UsersPage() -> impl IntoView {
    view! {
        <DataFormPage<UserTable>/>
    }
}
