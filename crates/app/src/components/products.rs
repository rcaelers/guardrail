use leptos::html::Div;
use leptos::*;
use leptos_struct_table::*;
use tracing::info;

use crate::data_provider::{ProductRow, ProductTableDataProvider};

#[component]
pub fn ProductsPage() -> impl IntoView {
    let scroll_container = create_node_ref::<Div>();

    let rows = ProductTableDataProvider::new();

    let name = rows.name;
    let (custom_text, set_custom_text) = create_signal("".to_string());
    let (show_popup, set_show_popup) = create_signal(false);

    let on_change = move |evt: ChangeEvent<ProductRow>| {
        info!("on_change {:?}", evt.changed_row);
        set_custom_text.set(format!("Remove product '{}'", evt.changed_row.name));
        set_show_popup.set(true);
    };

    let on_yes_click = move |_| {
        info!("Yes button clicked");
        set_show_popup(false);
    };

    let on_no_click = move |_| {
        info!("No button clicked");
        set_show_popup(false);
    };

    let on_add_click = move |_| {
        info!("Add button clicked");
        set_show_popup(false);
    };

    view! {
        <div class="border-b bg-slate-100 px-5 py-2">
            <label class="relative block">
                <span class="absolute inset-y-0 left-0 flex items-center pl-3">
                    <svg
                        class="h-5 w-5 fill-black"
                        xmlns="http://www.w3.org/2000/svg"
                        x="0px"
                        y="0px"
                        width="30"
                        height="30"
                        viewBox="0 0 30 30"
                    >
                        <path d="M 13 3 C 7.4889971 3 3 7.4889971 3 13 C 3 18.511003 7.4889971 23 13 23 C 15.396508 23 17.597385 22.148986 19.322266 20.736328 L 25.292969 26.707031 A 1.0001 1.0001 0 1 0 26.707031 25.292969 L 20.736328 19.322266 C 22.148986 17.597385 23 15.396508 23 13 C 23 7.4889971 18.511003 3 13 3 z M 13 5 C 17.430123 5 21 8.5698774 21 13 C 21 17.430123 17.430123 21 13 21 C 8.5698774 21 5 17.430123 5 13 C 5 8.5698774 8.5698774 5 13 5 z"></path>
                    </svg>
                </span>
                <input
                    class="bg-white placeholder:font-italics border border-slate-300 rounded-full py-2 pl-10 pr-4 focus:outline-none"
                    placeholder="Search"
                    type="text"
                    value=name
                    on:change=move |e| name.set(event_target_value(&e))
                />
                <button class="px-4 py-2 bg-gray-300 text-gray-700 rounded" on:click=on_add_click>
                    "Add Product"
                </button>
            </label>
        </div>
        <div node_ref=scroll_container class="overflow-auto grow min-h-0">
            <table class="table-fixed text-sm text-left text-gray-500 dark:text-gray-400 w-full">
                <TableContent rows on_change scroll_container/>
            </table>
        </div>

        {move || {
            if show_popup.get() {
                view! {
                    // This div covers the entire viewport with a semi-transparent background
                    <div class="fixed inset-0 flex items-center justify-center bg-zinc-900 bg-opacity-50">
                        // This div is the actual modal box
                        <div class="bg-white rounded-lg shadow-lg p-6 w-1/3">
                            <h2 class="text-lg text-gray-700 font-semibold">{custom_text.get()}</h2>
                            <h3 class="text-lg text-gray-700 font-semibold">"Are you sure?"</h3>
                            <div class="mt-4 flex justify-end space-x-4">
                                <button
                                    class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                                    on:click=on_no_click
                                >
                                    "No"
                                </button>
                                <button
                                    class="px-4 py-2 bg-red-500 text-white rounded"
                                    on:click=on_yes_click
                                >
                                    "Yes"
                                </button>
                            </div>
                        </div>
                    </div>
                }
                    .into_view()
            } else {
                view! {}.into_view()
            }
        }}
    }
}
