use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;

use enumflags2::{bitflags, BitFlags};
use indexmap::IndexMap;
use leptos::html::Div;
use leptos::*;
use leptos_router::*;
use leptos_struct_table::*;
use tracing::info;
use uuid::Uuid;

use crate::components::confirmation::ConfirmationModal;
use crate::components::form::{Field, FormModal};
use crate::components::header::Header;
use crate::data::QueryParams;
use crate::data_providers::{ExtraRowTrait, ExtraTableDataProvider};

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
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Capabilities {
    CanEdit = 0b0001,
    CanAdd = 0b0010,
    CanDelete = 0b0100,
}

#[trait_variant::make(DataFormTrait: Send)]
pub trait LocalDataFormTrait {
    type RowType: leptos_struct_table::TableRow + ExtraRowTrait + Clone + 'static;
    type TableDataProvider: leptos_struct_table::TableDataProvider<Self::RowType>
        + ExtraTableDataProvider<Self::RowType>
        + Clone
        + 'static;
    type DataType: Default + Clone + Debug + 'static;

    fn new_provider(parents: HashMap<String, Uuid>) -> Self::TableDataProvider;

    fn capabilities() -> BitFlags<Capabilities, u8> {
        Capabilities::CanEdit | Capabilities::CanAdd | Capabilities::CanDelete
    }

    fn get_related() -> Vec<Related> {
        vec![]
    }

    fn get_foreign() -> Vec<Foreign> {
        vec![]
    }

    fn get_data_type_name() -> String;

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, parent: HashMap<String, Uuid>);
    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, data: Self::DataType);
    fn update_data(
        data: &mut Self::DataType,
        fields: RwSignal<IndexMap<String, Field>>,
        parent_id: HashMap<String, Uuid>,
    );

    async fn list(
        parents: HashMap<String, Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Self::DataType>, ServerFnError<String>>;

    async fn get(id: Uuid) -> Result<Self::DataType, ServerFnError<String>>;
    async fn list_names(
        parents: HashMap<String, Uuid>,
    ) -> Result<HashSet<String>, ServerFnError<String>>;
    async fn add(data: Self::DataType) -> Result<(), ServerFnError<String>>;
    async fn update(data: Self::DataType) -> Result<(), ServerFnError<String>>;
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>>;
    async fn count(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError<String>>;
}

#[allow(non_snake_case)]
#[component]
pub fn DataFormPage<T: DataFormTrait>(#[prop(optional)] _ty: PhantomData<T>) -> impl IntoView {
    let query_map = use_query_map();

    let mut query = HashMap::new();
    for foreign in T::get_foreign() {
        let q = query_map.get_untracked();
        let q = q.get(foreign.query.as_str());
        if let Some(q) = q {
            info!("{}: {}", foreign.query, q);
            let uuid = uuid::Uuid::parse_str(q);
            if let Ok(uuid) = uuid {
                info!("{}: {}", foreign.id_name, uuid);
                query.insert(foreign.id_name, uuid);
            }
        }
    }

    let fields: RwSignal<IndexMap<String, Field>> = create_rw_signal(IndexMap::new());

    let title = create_rw_signal("".to_string());
    let related = create_rw_signal(T::get_related());
    let capabilities = create_rw_signal(T::capabilities());

    let scroll_container = create_node_ref::<Div>();
    let rows = <T as DataFormTrait>::new_provider(query.clone());
    let rows_clone = rows.clone();

    let selected_index: RwSignal<Option<usize>> = create_rw_signal(None);
    let (selected_row, set_selected_row) = create_signal(None);

    let filter = rows.get_filter_signal();
    let (custom_text, set_custom_text) = create_signal("".to_string());
    let (show_confirm_popup, set_show_confirm_popup) = create_signal(false);
    let (show_form_popup, set_show_form_popup) = create_signal(false);

    #[derive(Debug, Clone)]
    enum State {
        Idle,
        Add,
        Edit,
        Delete,
    }
    let state = create_rw_signal(State::Idle);

    let current_row: RwSignal<Option<T::DataType>> = create_rw_signal(None);
    let is_row_selected = create_memo(move |_| selected_row.get().is_some());

    T::initial_fields(fields, query.clone());

    create_effect(move |_| {
        if let State::Idle = state.get() {
            let rows = rows.clone();
            rows.update();
        }
    });

    let on_delete_click = Callback::new(move |_evt: web_sys::MouseEvent| {
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

    let on_related_click = Callback::new(move |index: usize| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            let id = row.get_id();
            spawn_local(async move {
                let navigate = use_navigate();
                let foreign = T::get_related();
                let foreign = foreign.get(index);

                if let Some(foreign) = foreign {
                    navigate(
                        format!("{}{}", foreign.url, id).as_str(),
                        Default::default(),
                    );
                }
            });
        }
    });

    let on_add_click = move |_: web_sys::MouseEvent| {
        let data = T::DataType::default();
        T::update_fields(fields, data);
        state.set(State::Add);
        title.set(format!("Add {}", T::get_data_type_name()));
        set_show_form_popup.set(true);
    };

    let on_edit_click = Callback::new(move |_: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            spawn_local(async move {
                let data: T::DataType = T::get(row.get_id()).await.unwrap();
                current_row.set(Some(data.clone()));
                T::update_fields(fields, data);
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

    let on_no_click = move |_| {
        set_show_confirm_popup(false);
    };

    let on_save_click = Callback::new(move |_| {
        set_show_form_popup(false);

        match state.get() {
            State::Add => {
                let mut data = T::DataType::default();
                T::update_data(&mut data, fields, query.clone());
                spawn_local(async move {
                    T::add(data).await.unwrap();
                    state.set(State::Idle);
                });
            }
            State::Edit => {
                let mut data = current_row.get().unwrap();
                T::update_data(&mut data, fields, query.clone());
                spawn_local(async move {
                    T::update(data).await.unwrap();
                    state.set(State::Idle);
                });
            }
            _ => {}
        }
    });

    let on_cancel_click = move |_| {
        set_show_form_popup(false);
        state.set(State::Idle);
    };

    let on_selection_changed = move |evt: SelectionChangeEvent<T::RowType>| {
        set_selected_row.update(|selected_row| {
            *selected_row = Some(evt.row);
        })
    };

    view! {
        <Header
            filter=filter
            capabilities=capabilities
            enabled=is_row_selected
            related=related
            on_edit_click=on_edit_click
            on_add_click=on_add_click.into()
            on_delete_click=on_delete_click
            on_related_click=on_related_click
        />

        <div node_ref=scroll_container class="overflow-auto grow min-h-0">
            <table class="table-fixed text-sm text-left text-gray-500 dark:text-gray-400 w-full">
                <TableContent
                    rows=rows_clone
                    scroll_container
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
            on_no_click=on_no_click.into()
        />

        <FormModal
            title=title
            show=show_form_popup
            fields=fields
            on_save_click=on_save_click
            on_cancel_click=on_cancel_click.into()
        />
    }
}
