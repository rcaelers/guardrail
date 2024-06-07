use std::collections::HashSet;

use ::chrono::Utc;
use leptos::html::Div;
use leptos::*;
use leptos_struct_table::*;
use tracing::info;
use uuid::Uuid;

use crate::components::confirmation::ConfirmationModal;
use crate::data::{product_add, product_get, product_list_names, product_update, Product};
use crate::data_provider::{ProductRow, ProductTableDataProvider};

#[allow(non_snake_case)]
#[component]
fn ProductFormModal(
    show: ReadSignal<bool>,
    product_name: RwSignal<String>,
    existing_product_names: RwSignal<HashSet<String>>,
    on_save_click: Callback<()>,
    on_cancel_click: Callback<()>,
) -> impl IntoView {
    let local_product_name = create_rw_signal("".to_string());
    create_effect(move |_| {
        info!("Product name effect");
        if show.get() {
            info!("Product name effect get");
            local_product_name.set(product_name.get());
        }
    });

    let product_exists = create_memo(move |_| {
        info!("product_exists");
        existing_product_names
            .get()
            .contains(&local_product_name.get())
            && local_product_name.get() != product_name.get()
    });

    view! {
        {move || {
            if show.get() {
                view! {
                    <div class="fixed inset-0 flex items-center justify-center bg-gray-900 bg-opacity-50">
                        <div class="modal modal-open">
                            <div class="modal-box">
                                <h2 class="font-bold text-lg">"New Product"</h2>
                                <div class="mt-4">
                                    <label class="block text-sm font-medium text-gray-700">
                                        "Product Name"
                                    </label>
                                    <input
                                        type="text"
                                        // class={input_class()}
                                        class:input-error=move || product_exists.get()
                                        class="input input-bordered w-full mt-1"
                                        placeholder="Enter product name"
                                        value=product_name.get()
                                        on:input=move |ev| {
                                            local_product_name.set(event_target_value(&ev))
                                        }
                                    />
                                </div>
                                <div class="modal-action">
                                    <button class="btn" on:click=move |_| on_cancel_click(())>
                                        "Cancel"
                                    </button>
                                    <button
                                        class="btn btn-primary"
                                        class:btn-disabled=move || product_exists.get()
                                        on:click=move |_| {
                                            product_name.set(local_product_name.get());
                                            on_save_click(())
                                        }
                                    >

                                        "Save"
                                    </button>
                                </div>
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

#[component]
pub fn ProductsPage() -> impl IntoView {
    let scroll_container = create_node_ref::<Div>();

    let rows = ProductTableDataProvider::new();
    let selected_index = create_rw_signal(None);
    let (selected_row, set_selected_row) = create_signal(None);

    let name = rows.get_name_signal();
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

    let product_name = create_rw_signal("".to_string());
    let current_row: RwSignal<Option<Product>> = create_rw_signal(None);

    let existing_product_names = create_rw_signal(HashSet::new());

    create_effect(move |_| {
        spawn_local(async move {
            info!("Fetching product names");
            match product_list_names().await {
                Ok(fetched_names) => existing_product_names.set(fetched_names),
                Err(e) => tracing::error!("Failed to fetch product names: {:?}", e),
            }
        });
    });

    create_effect(move |_| {
        info!("Show value: {}", show_form_popup.get());
    });

    let on_delete_click = move |_evt: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: ProductRow = row.unwrap();
            spawn_local(async move {
                let product = product_get(row.id).await.unwrap();
                info!("Delete product '{}' '{}'", row.id, product.name);
                set_custom_text.set(format!("Remove product '{}'", row.name));
                set_show_confirm_popup.set(true);
            });
        }
    };

    let on_related_click = move |_evt: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: ProductRow = row.unwrap();
            spawn_local(async move {
                let product = product_get(row.id).await.unwrap();
                info!("Related product '{}' '{}'", row.id, product.name);
            });
        }
    };

    let on_add_click = move |_| {
        info!("Add button clicked");
        product_name.set("".to_string());
        state.set(State::Add);
        set_show_form_popup.set(true);
    };

    let on_edit_click = move |_| {
        info!("Edit button clicked");
        let row = selected_row.get();
        if row.is_some() {
            let row: ProductRow = row.unwrap();
            spawn_local(async move {
                let product = product_get(row.id).await.unwrap();
                current_row.set(Some(product.clone()));
                info!("Edit product '{}' '{}'", row.id, product.name);
                product_name.set(product.name);
                state.set(State::Edit);
                set_show_form_popup.set(true);
            });
        }
    };

    let rows2 = rows.clone();
    let on_yes_click = move |_| {
        info!("Yes button clicked");
        set_show_confirm_popup(false);
        if let State::Delete = state.get() {
            info!("Delete")
        }
    };

    let on_no_click = move |_| {
        info!("No button clicked");
        set_show_confirm_popup(false);
    };

    let on_save_click = move |_| {
        info!("Save button clicked");
        set_show_form_popup(false);

        match state.get() {
            State::Idle => {
                info!("Idle")
            }
            State::Add => {
                info!("Adding product '{}'", product_name.get());
                let r = Product {
                    id: Uuid::new_v4(),
                    name: product_name.get(),
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                };
                spawn_local(async move {
                    product_add(r).await.unwrap();
                });
            }
            State::Edit => {
                info!("Updating product '{}'", product_name.get());
                let mut r = current_row.get().unwrap();
                r.name = product_name.get();
                spawn_local(async move {
                    info!("Updating product '{:?}'", r);
                    product_update(r).await.unwrap();
                });
            }
            _ => {
                info!("Other")
            }
        }
        state.set(State::Idle);
        rows2.update();
    };

    let on_cancel_click = move |_| {
        info!("Cancel button clicked");
        set_show_form_popup(false);
        state.set(State::Idle);
    };

    let on_selection_changed = move |evt: SelectionChangeEvent<ProductRow>| {
        set_selected_row.update(|selected_row| {
            *selected_row = Some(evt.row);
        })
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
                <button
                    class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                    class:button-disabled=move || selected_row.get().is_some()
                    on:click=on_edit_click
                >
                    "Edit"
                </button>
                <button
                    class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                    on:click=on_delete_click
                >
                    "Delete"
                </button>
                <button
                    class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                    on:click=on_related_click
                >
                    "Show Versions"
                </button>
            </label>
        </div>

        <div node_ref=scroll_container class="overflow-auto grow min-h-0">
            <table class="table-fixed text-sm text-left text-gray-500 dark:text-gray-400 w-full">
                <TableContent
                    rows
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
            on_yes_click=on_yes_click.into()
            on_no_click=on_no_click.into()
        />
        <ProductFormModal
            show=show_form_popup
            product_name=product_name
            existing_product_names=existing_product_names
            on_save_click=on_save_click.into()
            on_cancel_click=on_cancel_click.into()
        />
    }
}
