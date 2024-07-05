use indexmap::IndexMap;
use leptos::*;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub readonly: RwSignal<bool>,
    pub value: RwSignal<String>,
    pub multiselect: RwSignal<Vec<String>>,
    pub disallowed: RwSignal<HashSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInternal {
    pub initial_value: RwSignal<String>,
    pub valid: Memo<bool>,
}

impl From<Field> for FieldInternal {
    fn from(field: Field) -> Self {
        let mut internal = FieldInternal {
            initial_value: create_rw_signal(field.value.get_untracked()),
            valid: create_memo(move |_| true),
        };
        internal.valid = create_memo(move |_| {
            !field.disallowed.get().contains(&field.value.get())
                || (!internal.initial_value.get().is_empty()
                    && internal.initial_value.get() == field.value.get())
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
        fields.with_untracked(|fields| {
            fields
                .iter()
                .map(|(k, v)| {
                    let field = FieldInternal::from(v.clone());
                    (k.clone(), field)
                })
                .collect::<IndexMap<String, FieldInternal>>()
        })
    });

    let valid = create_memo(move |_| fields_internal().values().all(|field| field.valid.get()));

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
                                    key=|internal_field| internal_field.0.clone()
                                    children=move |internal_field| {
                                        let field = fields
                                            .get()
                                            .get(&internal_field.0)
                                            .unwrap()
                                            .clone();
                                        view! {
                                            <div class="mt-4">
                                                <label class="block text-sm font-medium text-gray-700">
                                                    {internal_field.0}
                                                </label>
                                                {if !field.multiselect.get().is_empty() {
                                                    view! {
                                                        <select
                                                            class="select select-bordered w-full mt-1"
                                                            on:change=move |ev| {
                                                                field
                                                                    .value
                                                                    .update(|data| {
                                                                        *data = event_target_value(&ev);
                                                                    });
                                                            }
                                                        >

                                                            <For
                                                                each=field.multiselect
                                                                key=|name| name.clone()
                                                                children=move |name| {
                                                                    let name_clone = name.clone();
                                                                    view! {
                                                                        <option selected=move || {
                                                                            field.value.get() == name
                                                                        }>{name_clone}</option>
                                                                    }
                                                                }
                                                            />

                                                        </select>
                                                    }
                                                        .into_view()
                                                } else {
                                                    view! {
                                                        <input
                                                            type="text"
                                                            class:input-error=move || !internal_field.1.valid.get()
                                                            class="input input-bordered w-full mt-1"
                                                            value=field.value.get()
                                                            disabled=move || field.readonly.get()
                                                            on:input=move |ev| {
                                                                field.value.set(event_target_value(&ev))
                                                            }
                                                        />
                                                    }
                                                        .into_view()
                                                }}

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
