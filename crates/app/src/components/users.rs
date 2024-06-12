use std::collections::HashSet;

use ::chrono::Utc;
use leptos::html::Div;
use leptos::*;
use leptos_struct_table::*;
use tracing::info;
use uuid::Uuid;

use crate::components::confirmation::ConfirmationModal;
use crate::data::{user_add, user_get, user_list_names, user_update, User};
use crate::data_providers::user::{UserRow, UserTableDataProvider};

#[allow(non_snake_case)]
#[component]
fn UserFormModal(
    show: ReadSignal<bool>,
    user_name: RwSignal<String>,
    existing_user_names: RwSignal<HashSet<String>>,
    on_save_click: Callback<()>,
    on_cancel_click: Callback<()>,
) -> impl IntoView {
    let local_user_name = create_rw_signal("".to_string());
    create_effect(move |_| {
        info!("User name effect");
        if show.get() {
            info!("User name effect get");
            local_user_name.set(user_name.get());
        }
    });

    let user_exists = create_memo(move |_| {
        info!("user_exists");
        existing_user_names.get().contains(&local_user_name.get())
            && local_user_name.get() != user_name.get()
    });

    view! {
        {move || {
            if show.get() {
                view! {
                    <div class="fixed inset-0 flex items-center justify-center bg-gray-900 bg-opacity-50">
                        <div class="modal modal-open">
                            <div class="modal-box">
                                <h2 class="font-bold text-lg">"New User"</h2>
                                <div class="mt-4">
                                    <label class="block text-sm font-medium text-gray-700">
                                        "User Name"
                                    </label>
                                    <input
                                        type="text"
                                        // class={input_class()}
                                        class:input-error=move || user_exists.get()
                                        class="input input-bordered w-full mt-1"
                                        placeholder="Enter user name"
                                        value=user_name.get()
                                        on:input=move |ev| {
                                            local_user_name.set(event_target_value(&ev))
                                        }
                                    />

                                </div>
                                <div class="modal-action">
                                    <button class="btn" on:click=move |_| on_cancel_click(())>
                                        "Cancel"
                                    </button>
                                    <button
                                        class="btn btn-primary"
                                        class:btn-disabled=move || user_exists.get()
                                        on:click=move |_| {
                                            user_name.set(local_user_name.get());
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

#[allow(non_snake_case)]
#[component]
fn MultiSelect() -> impl IntoView {
    let initial_items = vec![
        "Item 1".to_string(),
        "Item 2".to_string(),
        "Item 3".to_string(),
        "Item 4".to_string(),
    ];

    let (selected_items, set_selected_items) = create_signal(vec![]);
    let (input_value, set_input_value) = create_signal("".to_string());

    let add_item = move |item: String| {
        if !selected_items.get().contains(&item) {
            set_selected_items.update(|items| items.push(item.clone()));
            set_input_value.set("".to_string()); // Clear input value after adding item
        }
    };

    let remove_item = move |item: String| {
        set_selected_items.update(|items| items.retain(|i| i != &item));
    };

    let filtered_items = move || {
        let input = input_value.get();
        initial_items
            .iter()
            .filter(|&item| item.to_lowercase().contains(&input.to_lowercase()))
            .filter(|&item| !selected_items.get().contains(item))
            .cloned()
            .collect::<Vec<_>>()
    };

    let handle_input = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_input_value.set(value);
    };

    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            let value = input_value.get();
            if !value.is_empty() {
                add_item(value.clone());
            }
        }
    };

    view! {
        <div class="p-4">
            <div class="flex flex-wrap space-x-2 mb-4 items-center border border-gray-300 p-2 rounded">
                <For
                    each=selected_items
                    key=|item| item.clone()
                    children=move |item| {
                        let item_clone = item.clone();
                        view! {
                            <div class="bg-blue-500 text-white rounded-full px-4 py-1 flex items-center space-x-2">
                                <span>{item_clone}</span>
                                <button
                                    class="text-white"
                                    on:click=move |_| remove_item(item.clone())
                                >
                                    {"x"}
                                </button>
                            </div>
                        }
                    }
                />

                <input
                    type="text"
                    placeholder="Add item"
                    class="input input-bordered flex-grow"
                    value=input_value.get()
                    on:input=handle_input
                    on:keydown=handle_keydown
                />
            </div>
            <ul class="absolute bg-white shadow-lg rounded-lg mt-1 w-full max-h-40 overflow-y-auto z-10">
                <For
                    each=filtered_items
                    key=|item| item.clone()
                    children=move |item| {
                        let item_clone = item.clone();
                        view! {
                            <li
                                class="px-4 py-2 hover:bg-gray-100 cursor-pointer"
                                on:click=move |_| add_item(item.clone())
                            >
                                {item_clone}
                            </li>
                        }
                    }
                />

            </ul>
        </div>
    }
}

#[allow(non_snake_case)]
#[component]
pub fn UsersPage() -> impl IntoView {
    let scroll_container = create_node_ref::<Div>();

    let rows = UserTableDataProvider::new();
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

    let user_name = create_rw_signal("".to_string());
    let current_row: RwSignal<Option<User>> = create_rw_signal(None);

    let existing_user_names = create_rw_signal(HashSet::new());

    create_effect(move |_| {
        spawn_local(async move {
            info!("Fetching user names");
            match user_list_names().await {
                Ok(fetched_names) => existing_user_names.set(fetched_names),
                Err(e) => tracing::error!("Failed to fetch user names: {:?}", e),
            }
        });
    });

    create_effect(move |_| {
        info!("Show value: {}", show_form_popup.get());
    });

    let on_delete_click = move |_evt: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: UserRow = row.unwrap();
            spawn_local(async move {
                let user = user_get(row.id).await.unwrap();
                info!("Delete user '{}' '{}'", row.id, user.username);
                set_custom_text.set(format!("Remove user '{}'", row.username));
                set_show_confirm_popup.set(true);
            });
        }
    };

    let on_related_click = move |_evt: web_sys::MouseEvent| {
        let row = selected_row.get();
        if row.is_some() {
            let row: UserRow = row.unwrap();
            spawn_local(async move {
                let user = user_get(row.id).await.unwrap();
                info!("Related user '{}' '{}'", row.id, user.username);
            });
        }
    };

    let on_add_click = move |_| {
        info!("Add button clicked");
        user_name.set("".to_string());
        state.set(State::Add);
        set_show_form_popup.set(true);
    };

    let on_edit_click = move |_| {
        info!("Edit button clicked");
        let row = selected_row.get();
        if row.is_some() {
            let row: UserRow = row.unwrap();
            spawn_local(async move {
                let user = user_get(row.id).await.unwrap();
                current_row.set(Some(user.clone()));
                info!("Edit user '{}' '{}'", row.id, user.username);
                user_name.set(user.username);
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
                info!("Adding user '{}'", user_name.get());
                let r = User {
                    id: Uuid::new_v4(),
                    username: user_name.get(),
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                    last_login_at: None,
                    roles: vec![],
                };
                spawn_local(async move {
                    user_add(r).await.unwrap();
                });
            }
            State::Edit => {
                info!("Updating user '{}'", user_name.get());
                let mut r = current_row.get().unwrap();
                r.username = user_name.get();
                spawn_local(async move {
                    info!("Updating user '{:?}'", r);
                    user_update(r).await.unwrap();
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

    let on_selection_changed = move |evt: SelectionChangeEvent<UserRow>| {
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
                    "Add User"
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
        <UserFormModal
            show=show_form_popup
            user_name=user_name
            existing_user_names=existing_user_names
            on_save_click=on_save_click.into()
            on_cancel_click=on_cancel_click.into()
        />
    }
}
