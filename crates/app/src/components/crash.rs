use indexmap::IndexMap;
use leptos::*;
use leptos_router::*;

use crate::components::datatable_form::Field;

#[allow(non_snake_case)]
#[component]
pub fn Crash() -> impl IntoView {
    let query_map = use_query_map();

    let q = query_map.get_untracked();
    let q = q.get("crash").unwrap();
    let uuid = uuid::Uuid::parse_str(q).unwrap();

    let fields: RwSignal<IndexMap<String, Field>> = create_rw_signal(IndexMap::new());

    view! {
        // <Header
        //     filter=filter
        //     capabilities=capabilities
        //     enabled=is_row_selected
        //     related=related
        //     on_edit_click=on_edit_click
        //     on_add_click=on_add_click.into()
        //     on_delete_click=on_delete_click
        //     on_related_click=on_related_click
        // />

        // <ConfirmationModal
        //     show=show_confirm_popup
        //     custom_text=custom_text
        //     on_yes_click=on_yes_click
        //     on_no_click=on_no_click.into()
        // />

    }
}
