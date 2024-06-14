use enumflags2::BitFlags;
use ev::MouseEvent;
use leptos::*;

use super::dataform::{Capabilities, Related};

#[allow(non_snake_case)]
#[component]
pub fn Header(
    filter: RwSignal<String>,
    enabled: Memo<bool>,
    capabilities: RwSignal<BitFlags<Capabilities, u8>>,
    related: RwSignal<Vec<Related>>,
    on_add_click: Callback<MouseEvent>,
    on_edit_click: Callback<MouseEvent>,
    on_delete_click: Callback<MouseEvent>,
    on_related_click: Callback<usize>,
) -> impl IntoView {
    view! {
        <header class="sticky top-0 z-40 pb-1">
            <div class="flex items-center justify-between w-full">
                <div class="relative w-1/3">
                    <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 16 16"
                            fill="currentColor"
                            class="w-4 h-4 opacity-70"
                        >
                            <path
                                d="M9.965 11.026a5 5 0 1 1 1.06-1.06l2.755 2.754a.75.75 0 1 1-1.06 1.06l-2.755-2.754ZM10.5 7a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0Z"
                                fill-rule="evenodd"
                                clip-rule="evenodd"
                            ></path>
                        </svg>
                    </div>
                    <input
                        type="text"
                        class="input input-bordered pl-10 w-full"
                        placeholder="Search..."
                        value=filter
                        on:change=move |e| filter.set(event_target_value(&e))
                    />
                </div>

                <div class="flex space-x-2">
                    <button class="btn btn-primary"
                            class:hidden=move || !capabilities.get().contains(Capabilities::CanAdd)
                            on:click=on_add_click
                    >
                        "Add"
                    </button>
                    <button
                        class="btn btn-primary"
                        class:btn-disabled=move || !enabled.get()
                        class:hidden=move || !capabilities.get().contains(Capabilities::CanEdit)
                        on:click=on_edit_click
                    >
                        "Edit"
                    </button>
                    <button
                        class="btn btn-primary"
                        class:btn-disabled=move || !enabled.get()
                        class:hidden=move || !capabilities.get().contains(Capabilities::CanDelete)
                        on:click=on_delete_click
                    >
                        "Delete"
                    </button>
                    <For
                        each=move || {related.get().into_iter().enumerate().collect::<Vec<_>>()}
                        key=|(_index,related)| related.clone()
                        children=move |(index, related)| {
                        view! {
                            <button
                                class="btn btn-primary"
                                class:btn-disabled=move || !enabled.get()
                                on:click=move|_| { on_related_click(index); }
                            >
                                "Show " { related.name }
                            </button>
                        }
                    }
                    />
                </div>
            </div>
        </header>
    }
}
