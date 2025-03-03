use dyn_clone::DynClone;
use indexmap::IndexMap;
use leptos::prelude::*;
use std::any::Any;
use std::collections::HashSet;
use std::fmt::Debug;

pub trait FieldValueTrait: Debug + Sync + Send + DynClone {
    fn render(&self, options: FieldOptions) -> AnyView;
    fn valid(&self) -> Memo<bool> {
        Memo::new(move |_| true)
    }
    fn as_any(&self) -> &dyn Any;
}
dyn_clone::clone_trait_object!(FieldValueTrait);

pub trait FieldTrait: Debug + Sync + Send + DynClone {
    fn render(&self, options: FieldOptions) -> AnyView;
    fn valid(&self) -> Memo<bool>;
    fn value(&self) -> &dyn FieldValueTrait;
    fn options(&self) -> &FieldOptions;
}
dyn_clone::clone_trait_object!(FieldTrait);

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldOptions {
    pub readonly: RwSignal<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldString {
    pub value: RwSignal<String>,
    pub disallowed: RwSignal<HashSet<String>>,
    initial_value: RwSignal<String>,
    valid: Memo<bool>,
}

impl FieldString {
    pub fn new(value: String, disallowed: HashSet<String>) -> Self {
        let mut field = FieldString {
            value: RwSignal::new(value.clone()),
            disallowed: RwSignal::new(disallowed),
            initial_value: RwSignal::new(value.clone()),
            valid: Memo::new(move |_| true),
        };
        field.valid = Memo::new(move |_| {
            !field.disallowed.get().contains(&value)
                || (!field.initial_value.get().is_empty()
                    && field.initial_value.get() == field.value.get())
        });

        field
    }
}

impl Default for FieldString {
    fn default() -> Self {
        FieldString {
            value: RwSignal::new("".to_string()),
            disallowed: RwSignal::new(HashSet::new()),
            initial_value: RwSignal::new("".to_string()),
            valid: Memo::new(move |_| true),
        }
    }
}
impl FieldValueTrait for FieldString {
    fn render(&self, options: FieldOptions) -> AnyView {
        let valid = self.valid;
        let value = self.value;
        let readonly = options.readonly;
        view! {
            <input
                type="text"
                class:input-error=move || !valid.get()
                class="input input-bordered w-full mt-1"
                value=value.get()
                disabled=move || readonly.get()
                on:input=move |ev| { value.set(event_target_value(&ev)) }
            />
        }
        .into_any()
    }
    fn valid(&self) -> Memo<bool> {
        self.valid
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldCombo {
    pub value: RwSignal<String>,
    pub multiselect: RwSignal<Vec<String>>,
}

impl FieldValueTrait for FieldCombo {
    fn render(&self, _options: FieldOptions) -> AnyView {
        let value = self.value;
        let multiselect = self.multiselect;

        view! {
            <select
                class="select select-bordered w-full mt-1"
                on:change=move |ev| {
                    value
                        .update(|data| {
                            *data = event_target_value(&ev);
                        });
                }
            >

                <For
                    each=multiselect
                    key=|name| name.clone()
                    children=move |name| {
                        let name_clone = name.clone();
                        view! {
                            <option selected=move || {
                                value.get() == name.clone()
                            }>{name_clone}</option>
                        }
                    }
                />

            </select>
        }
        .into_any()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldCheckbox {
    pub value: RwSignal<bool>,
}

impl FieldCheckbox {
    pub fn new(value: bool) -> Self {
        FieldCheckbox {
            value: RwSignal::new(value),
        }
    }
}

impl FieldValueTrait for FieldCheckbox {
    fn render(&self, options: FieldOptions) -> AnyView {
        let value = self.value;
        let readonly = options.readonly;
        view! {
            <input
                type="checkbox"
                class="form-checkbox h-5 w-5 text-blue-600"
                checked=value.get()
                disabled=readonly.get()
                on:change=move |ev| {
                    value
                        .update(|data| {
                            *data = event_target_checked(&ev);
                        });
                }
            />
        }
        .into_any()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub struct Field<T: FieldValueTrait> {
    pub options: FieldOptions,
    pub value: T,
}

impl<T: FieldValueTrait> Field<T> {
    pub fn new(value: T) -> Self {
        Field {
            options: FieldOptions::default(),
            value,
        }
    }
}

impl<T: Debug + Clone + FieldValueTrait + 'static> FieldTrait for Field<T> {
    fn render(&self, options: FieldOptions) -> AnyView {
        self.value.render(options)
    }

    fn valid(&self) -> Memo<bool> {
        self.value.valid()
    }

    fn value(&self) -> &dyn FieldValueTrait {
        &self.value
    }

    fn options(&self) -> &FieldOptions {
        &self.options
    }
}

impl<T: Debug + FieldValueTrait + Default> Default for Field<T> {
    fn default() -> Self {
        Field {
            options: FieldOptions::default(),
            value: T::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Fields(IndexMap<String, Box<dyn FieldTrait>>);

impl Fields {
    pub fn new() -> Self {
        Fields(IndexMap::new())
    }

    pub fn insert<T: FieldValueTrait + Clone + 'static>(&mut self, name: String, field: Field<T>) {
        self.0.insert(name, Box::new(field));
    }

    pub fn values(&self) -> indexmap::map::Values<String, Box<dyn FieldTrait>> {
        self.0.values()
    }

    pub fn get<T: FieldValueTrait + Clone + 'static>(&self, name: &str) -> T {
        self.0
            .get(name)
            .unwrap()
            .value()
            .as_any()
            .downcast_ref::<T>()
            .unwrap()
            .clone()
    }

    pub fn get_options(&self, name: &str) -> FieldOptions {
        self.0.get(name).unwrap().options().clone()
    }

    pub fn iter(&self) -> indexmap::map::Iter<String, Box<dyn FieldTrait>> {
        self.0.iter()
    }
}

impl IntoIterator for Fields {
    type Item = (String, Box<dyn FieldTrait>);
    type IntoIter = indexmap::map::IntoIter<String, Box<dyn FieldTrait>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[allow(non_snake_case)]
#[component]
pub fn DataTableModalForm(
    title: RwSignal<String>,
    show: ReadSignal<bool>,
    fields: RwSignal<Fields>,
    #[prop(into)] on_save_click: Callback<()>,
    #[prop(into)] on_cancel_click: Callback<()>,
) -> impl IntoView {
    let valid = Memo::new(move |_| fields.get().values().all(|field| field.valid().get()));

    move || {
        if show.get() {
            view! {
                <div class="fixed inset-0 flex items-center justify-center bg-gray-900 bg-opacity-50">
                    <div class="modal modal-open">
                        <div class="modal-box">
                            <h2 class="font-bold text-lg">{title}</h2>
                            <For
                                each=fields
                                key=|field| field.0.clone()
                                children=move |field| {
                                    view! {
                                        <div class="mt-4">
                                            <label class="block text-sm font-medium text-gray-700">
                                                {field.0}
                                            </label>
                                            {field.1.value().render(field.1.options().clone())}
                                        </div>
                                    }
                                }
                            />

                            <div class="modal-action">
                                <button class="btn" on:click=move |_| on_cancel_click.run(())>
                                    "Cancel"
                                </button>
                                <button
                                    class="btn btn-primary"
                                    class:btn-disabled=move || !valid.get()
                                    on:click=move |_| {
                                        on_save_click.run(());
                                    }
                                >

                                    "Save"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            }
                    .into_any()
        } else {
            view! {};
            ().into_any()
        }
    }
}
