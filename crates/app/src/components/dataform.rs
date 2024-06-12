use std::collections::HashSet;
use std::marker::PhantomData;

use indexmap::IndexMap;
use leptos::html::Div;
use leptos::*;
use leptos_router::*;
use leptos_struct_table::*;
use uuid::Uuid;

use crate::components::confirmation::ConfirmationModal;
use crate::components::form::{Field, FormModal};
use crate::components::header::Header;
use crate::data::QueryParams;
use crate::data_providers::{ExtraRowTrait, ExtraTableDataProvider};

pub trait ParamsTrait {
    fn get_id(self) -> String;
    fn get_param_name() -> String;
}

#[trait_variant::make(DataFormTrait: Send)]
pub trait LocalDataFormTrait {
    type RequestParams: leptos_router::Params + PartialEq + Clone + ParamsTrait + 'static;
    type RowType: leptos_struct_table::TableRow + ExtraRowTrait + Clone + 'static;
    type TableDataProvider: leptos_struct_table::TableDataProvider<Self::RowType>
        + ExtraTableDataProvider<Self::RowType>
        + Clone
        + 'static;
    type DataType: Default + Clone + 'static;

    fn new_provider(parent_id: Option<Uuid>) -> Self::TableDataProvider;
    fn get_data_type_name() -> String;
    fn get_related_name() -> Option<String>;
    fn get_related_url(parent_id: Uuid) -> String;

    fn initial_fields(fields: RwSignal<IndexMap<String, Field>>, parent_id: Option<uuid::Uuid>);
    fn update_fields(fields: RwSignal<IndexMap<String, Field>>, data: Self::DataType);
    fn update_data(data: &mut Self::DataType, fields: RwSignal<IndexMap<String, Field>>);

    async fn list(
        parent_id: Option<Uuid>,
        query_params: QueryParams,
    ) -> Result<Vec<Self::DataType>, ServerFnError<String>>;

    async fn get(id: Uuid) -> Result<Self::DataType, ServerFnError<String>>;
    async fn list_names(parent_id: Option<Uuid>) -> Result<HashSet<String>, ServerFnError<String>>;
    async fn add(data: Self::DataType) -> Result<(), ServerFnError<String>>;
    async fn update(data: Self::DataType) -> Result<(), ServerFnError<String>>;
    async fn remove(id: Uuid) -> Result<(), ServerFnError<String>>;
    async fn count(parent_id: Option<Uuid>) -> Result<usize, ServerFnError<String>>;
}

#[allow(non_snake_case)]
#[component]
pub fn DataFormPage<T: DataFormTrait>(#[prop(optional)] _ty: PhantomData<T>) -> impl IntoView {
    let params = use_params::<T::RequestParams>();

    let product_id = params
        .get_untracked()
        .map(|p| uuid::Uuid::parse_str(&p.get_id()).ok())
        .ok()
        .flatten();

    let fields: RwSignal<IndexMap<String, Field>> = create_rw_signal(IndexMap::new());

    let title = create_rw_signal("".to_string());
    let related_title = create_rw_signal(T::get_related_name());

    let scroll_container = create_node_ref::<Div>();
    let rows = <T as DataFormTrait>::new_provider(product_id);
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

    T::initial_fields(fields, product_id);

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

    let on_related_click = Callback::new(move |_evt: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: T::RowType = row.unwrap();
            spawn_local(async move {
                let navigate = use_navigate();
                navigate(
                    T::get_related_url(row.get_id()).as_str(),
                    Default::default(),
                );
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
                T::update_data(&mut data, fields);
                spawn_local(async move {
                    T::add(data).await.unwrap();
                    state.set(State::Idle);
                });
            }
            State::Edit => {
                let mut data = current_row.get().unwrap();
                T::update_data(&mut data, fields);
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
            enabled=is_row_selected
            related=related_title
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
