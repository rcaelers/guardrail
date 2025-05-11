use async_trait::async_trait;
use common::QueryParams;
use enumflags2::{BitFlags, bitflags};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;
use tracing::info;
use uuid::Uuid;

use crate::components::confirmation::ConfirmationModal;
use crate::components::datatable_form::{DataTableModalForm, Fields};
use crate::components::datatable_header::DataTableHeader;
use crate::data_providers::ExtraTableDataProvider;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Related {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Foreign {
    pub id_name: String,
    pub query: String,
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Capabilities {
    CanEdit = 0b0001,
    CanAdd = 0b0010,
    CanDelete = 0b0100,
}

pub trait ExtraRowTrait {
    fn get_id(&self) -> Uuid;
    fn get_name(&self) -> String;
}

#[async_trait]
pub trait DataTableTrait
where
    Self: leptos_struct_table::TableDataProvider<Self::RowType>
        + ExtraTableDataProvider<Self::RowType>
        + Clone
        + Sync
        + Send
        + 'static,
    <Self::RowType as leptos_struct_table::TableRow>::ClassesProvider: Send + Sync,
{
    type RowType: leptos_struct_table::TableRow
        + ExtraRowTrait
        + Sync
        + Send
        + Clone
        + Debug
        + 'static;
    type DataType: Default + Sync + Send + Clone + Debug + 'static;

    fn new_provider(parents: HashMap<String, Uuid>) -> Self;

    async fn capabilities(&self) -> BitFlags<Capabilities, u8>;

    fn get_related() -> Vec<Related> {
        vec![]
    }

    fn get_foreign() -> Vec<Foreign> {
        vec![]
    }

    fn get_columns() -> Vec<String> {
        vec![]
    }

    fn get_data_type_name() -> String;

    fn init_fields(fields: RwSignal<Fields>, parents: &HashMap<String, Uuid>);

    async fn update_fields(
        fields: RwSignal<Fields>,
        data: Self::DataType,
        parents: &HashMap<String, Uuid>,
    );
    fn update_data(
        data: &mut Self::DataType,
        fields: RwSignal<Fields>,
        parents: &HashMap<String, Uuid>,
    );

    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Self::DataType>, ServerFnError>;

    async fn get(id: Uuid) -> Result<Self::DataType, ServerFnError>;
    async fn list_names(parents: HashMap<String, Uuid>) -> Result<HashSet<String>, ServerFnError>;
    async fn add(data: Self::DataType) -> Result<(), ServerFnError>;
    async fn update(data: Self::DataType) -> Result<(), ServerFnError>;
    async fn remove(id: Uuid) -> Result<(), ServerFnError>;
    async fn count(parents: HashMap<String, Uuid>) -> Result<i64, ServerFnError>;
}

#[allow(non_snake_case)]
#[component]
pub fn DataTable<T>(#[prop(optional)] _ty: PhantomData<T>) -> impl IntoView
where
    T: DataTableTrait,
{
    let query_map = use_query_map();

    let mut query = HashMap::new();
    for foreign in T::get_foreign() {
        let q = query_map.get_untracked();
        let q = q.get(foreign.query.as_str());
        if let Some(q) = q {
            let uuid = uuid::Uuid::parse_str(q.as_str());
            if let Ok(uuid) = uuid {
                query.insert(foreign.id_name, uuid);
            }
        }
    }

    info!("DataTable: {:?}", query);
    let fields: RwSignal<Fields> = RwSignal::new(Fields::default());

    let title = RwSignal::new("".to_string());
    let related = RwSignal::new(T::get_related());

    let container = NodeRef::new();
    let form = T::new_provider(query.clone());
    let form_clone = form.clone();

    let rows_clone2 = form.clone();
    let capabilities = Resource::new(
        move || (),
        move |_| {
            let value = rows_clone2.clone();
            async move { value.capabilities().await }
        },
    );

    let selected_index: RwSignal<Option<usize>> = RwSignal::new(None);
    let (selected_row, set_selected_row) = signal(None);

    let filter = form.get_filter_signal();
    let (custom_text, set_custom_text) = signal("".to_string());
    let (show_confirm_popup, set_show_confirm_popup) = signal(false);
    let (show_form_popup, set_show_form_popup) = signal(false);

    #[derive(Debug, Clone)]
    enum State {
        Idle,
        Add,
        Edit,
        Delete,
    }
    let state = RwSignal::new(State::Idle);

    let current_row: RwSignal<Option<T::DataType>> = RwSignal::new(None);
    let is_row_selected = Memo::new(move |_| selected_row.get().is_some());

    T::init_fields(fields, &query);

    // Effect::new(move |_| {
    //     if let State::Idle = state.get() {
    //         let rows = form.clone();
    //         rows.refresh_table();
    //     }
    // });

    let on_delete_click = Callback::new(move |_| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            spawn_local(async move {
                set_custom_text.set(format!(
                    "Remove {} '{}'",
                    T::get_data_type_name(),
                    row.get_name()
                ));
                state.set(State::Delete);
                set_show_confirm_popup.set(true);
            });
        }
    });

    let on_related_click = Callback::new(move |(index,): (usize,)| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            let id = row.get_id();
            spawn_local(async move {
                let navigate = use_navigate();
                let foreign = T::get_related();
                let foreign = foreign.get(index);

                if let Some(foreign) = foreign {
                    navigate(format!("{}{}", foreign.url, id).as_str(), Default::default());
                }
            });
        }
    });

    let q1 = query.clone();
    let on_add_click = Callback::new(move |_| {
        let q1 = q1.clone();
        spawn_local(async move {
            let data: T::DataType = T::DataType::default();
            T::update_fields(fields, data, &q1).await;
            state.set(State::Add);
            title.set(format!("Add {}", T::get_data_type_name()));
            set_show_form_popup.set(true);
        });
    });

    let q2 = query.clone();
    let on_edit_click = Callback::new(move |_| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            let q2 = q2.clone();
            spawn_local(async move {
                let data: T::DataType = T::get(row.get_id()).await.unwrap();
                current_row.set(Some(data.clone()));
                T::update_fields(fields, data, &q2).await;
                title.set(format!("Edit {}", T::get_data_type_name()));
                state.set(State::Edit);
                set_show_form_popup.set(true);
            });
        }
    });

    let on_yes_click = Callback::new(move |_| {
        set_show_confirm_popup(false);
        if let State::Delete = state.get() {
            let row = selected_row.get();
            if row.is_some() {
                let row: T::RowType = row.unwrap();
                spawn_local(async move {
                    T::remove(row.get_id()).await.unwrap();
                    state.set(State::Idle);
                });
            }
        }
    });

    let on_no_click = Callback::new(move |_| {
        set_show_confirm_popup(false);
    });

    let on_save_click = Callback::new(move |_| {
        set_show_form_popup(false);

        match state.get() {
            State::Add => {
                let mut data = T::DataType::default();
                T::update_data(&mut data, fields, &query);
                spawn_local(async move {
                    T::add(data).await.unwrap();
                    state.set(State::Idle);
                });
            }
            State::Edit => {
                let mut data = current_row.get().unwrap();
                T::update_data(&mut data, fields, &query);
                spawn_local(async move {
                    T::update(data).await.unwrap();
                    state.set(State::Idle);
                });
            }
            _ => {}
        }
    });

    let on_cancel_click = Callback::new(move |_| {
        set_show_form_popup(false);
        state.set(State::Idle);
    });

    let on_selection_changed = move |evt: SelectionChangeEvent<T::RowType>| {
        set_selected_row.write().replace(evt.row.get_untracked().clone());
    };

    view! {
        <DataTableHeader
            filter=filter
            capabilities=capabilities
            enabled=is_row_selected
            related=related
            on_edit_click=on_edit_click
            on_add_click=on_add_click
            on_delete_click=on_delete_click
            on_related_click=on_related_click
        />

        <div node_ref=container class="overflow-auto grow min-h-0">
            <table class="table-fixed text-sm text-left text-gray-500 dark:text-gray-400 w-full">
                <TableContent
                    rows=form_clone
                    scroll_container=container
                    display_strategy=DisplayStrategy::Virtualization
                    selection=Selection::Single(selected_index)
                    on_selection_change=on_selection_changed
                />
            </table>
        </div>

        <ConfirmationModal
            show=show_confirm_popup
            custom_text=custom_text
            on_yes_click=on_yes_click
            on_no_click=on_no_click
        />

        <DataTableModalForm
            title=title
            show=show_form_popup
            fields=fields
            on_save_click=on_save_click
            on_cancel_click=on_cancel_click
        />
    }
}
