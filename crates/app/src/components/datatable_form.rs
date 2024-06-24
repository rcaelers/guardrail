use indexmap::IndexMap;
use leptos::*;
use std::collections::HashSet;
use tracing::info;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub value: RwSignal<String>,
    pub disallowed: RwSignal<HashSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInternal {
    pub value: RwSignal<String>,
    pub initial_value: RwSignal<String>,
    pub disallowed: RwSignal<HashSet<String>>,
    pub valid: Memo<bool>,
}

impl From<Field> for FieldInternal {
    fn from(field: Field) -> Self {
        let mut internal = FieldInternal {
            value: field.value,
            initial_value: create_rw_signal(field.value.get_untracked()),
            disallowed: field.disallowed,
            valid: create_memo(move |_| true),
        };
        internal.valid = create_memo(move |_| {
            info!(
                "valid check for {} {}",
                internal.value.get(),
                internal.disallowed.get().len()
            );
            !internal.disallowed.get().contains(&internal.value.get())
                || internal.initial_value.get() == internal.value.get()
        });

        internal
    }
}

#[allow(non_snake_case)]
#[component]
pub fn DataTableModalForm(
    title: RwSignal<String>,
    show: ReadSignal<bool>,
    fields: RwSignal<IndexMap<String, Field>>,
    on_save_click: Callback<()>,
    on_cancel_click: Callback<()>,
) -> impl IntoView {
    let fields_internal = create_memo(move |_| {
        fields.with(|fields| {
            fields
                .iter()
                .map(|(k, v)| {
                    let field = FieldInternal::from(v.clone());
                    (k.clone(), field)
                })
                .collect::<IndexMap<String, FieldInternal>>()
        })
    });

    let valid = create_memo(move |_| {
        fields_internal().values().all(|field| {
            info!(
                "valid check for {} {} {}",
                field.value.get(),
                field.disallowed.get().len(),
                field.valid.get()
            );
            field.valid.get()
        })
    });

    view! {
        {move || {
            if show.get() {
                view! {
                    <div class="fixed inset-0 flex items-center justify-center bg-gray-900 bg-opacity-50">
                        <div class="modal modal-open">
                            <div class="modal-box">
                                <h2 class="font-bold text-lg">{title}</h2>
                                <For
                                    each=fields_internal
                                    key=|field| field.0.clone()
                                    children=move |field| {
                                        view! {
                                            <div class="mt-4">
                                                <label class="block text-sm font-medium text-gray-700">
                                                    {field.0}
                                                </label>
                                                <input
                                                    type="text"
                                                    class:input-error=move || !field.1.valid.get()
                                                    class="input input-bordered w-full mt-1"
                                                    value=field.1.value.get()
                                                    on:input=move |ev| {
                                                        field.1.value.set(event_target_value(&ev))
                                                    }
                                                />

                                            </div>
                                        }
                                    }
                                />

                                <div class="modal-action">
                                    <button class="btn" on:click=move |_| on_cancel_click(())>
                                        "Cancel"
                                    </button>
                                    <button
                                        class="btn btn-primary"
                                        class:btn-disabled=move || !valid.get()
                                        on:click=move |_| { on_save_click(()) }
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
