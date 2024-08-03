use dyn_clone::DynClone;
use indexmap::IndexMap;
use leptos::*;
use std::any::Any;
use std::collections::HashSet;
use std::fmt::Debug;

pub trait FieldValueTrait: Debug + Send + DynClone {
    fn render(&self, options: FieldOptions) -> View;
    fn valid(&self) -> Memo<bool> {
        create_memo(move |_| true)
    }
    fn as_any(&self) -> &dyn Any;
}
dyn_clone::clone_trait_object!(FieldValueTrait);

pub trait FieldTrait: Debug + Send + DynClone {
    fn render(&self, options: FieldOptions) -> View;
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
            valid: create_memo(move |_| true),
        };
        field.valid = create_memo(move |_| {
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
            valid: create_memo(move |_| true),
        }
    }
}
impl FieldValueTrait for FieldString {
    fn render(&self, options: FieldOptions) -> View {
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
        .into_view()
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
    pub multiselect: RwSignal<HashSet<String>>,
}

impl FieldValueTrait for FieldCombo {
    fn render(&self, _options: FieldOptions) -> View {
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
        .into_view()
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
    fn render(&self, options: FieldOptions) -> View {
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
        .into_view()
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
    fn render(&self, options: FieldOptions) -> View {
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
    on_save_click: Callback<()>,
    on_cancel_click: Callback<()>,
) -> impl IntoView {
    let valid = create_memo(move |_| fields.get().values().all(|field| field.valid().get()));

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
    }
}

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct FieldInternal {
//     pub initial_value: RwSignal<FieldValue>,
//     pub valid: Memo<bool>,
// }

// impl From<Field> for FieldInternal {
//     fn from(field: Field) -> Self {
//         let mut internal = FieldInternal {
//             initial_value: create_rw_signal(field.value.get_untracked()),
//             valid: create_memo(move |_| true),
//         };
//         internal.valid = create_memo(move |_| {
//             if let FieldValue::String(value) = field.value.get() {
//                 !field.disallowed.get().contains(&value)
//                     || (!internal.initial_value.get().is_empty()
//                         && internal.initial_value.get() == field.value.get())
//             } else {
//                 true
//             }
//         });

//         internal
//     }
// }

// #[allow(non_snake_case)]
// #[component]
// fn StringField(field: Field, internal_field: FieldInternal) -> impl IntoView {
//     view! {
//         <input
//             type="text"
//             class:input-error=move || !internal_field.valid.get()
//             class="input input-bordered w-full mt-1"
//             value=field.value.get().as_string()
//             disabled=move || field.readonly.get()
//             on:input=move |ev| { field.value.set(event_target_value(&ev).into()) }
//         />
//     }
//     .into_view()
// }

// #[allow(non_snake_case)]
// #[component]
// fn SelectField(field: Field) -> impl IntoView {
//     view! {
//         <select
//             class="select select-bordered w-full mt-1"
//             on:change=move |ev| {
//                 field
//                     .value
//                     .update(|data| {
//                         *data = FieldValue::String(event_target_value(&ev));
//                     });
//             }
//         >

//             <For
//                 each=field.multiselect
//                 key=|name| name.clone()
//                 children=move |name| {
//                     let name_clone = name.clone();
//                     view! {
//                         <option selected=move || {
//                             field.value.get() == name.clone().into()
//                         }>{name_clone}</option>
//                     }
//                 }
//             />

//         </select>
//     }
// }

// #[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
// pub enum FieldValue {
//     #[default]
//     Void,
//     String(String),
//     Combo(String),
//     Bool(bool),
// }

// impl From<String> for FieldValue {
//     fn from(s: String) -> Self {
//         FieldValue::String(s)
//     }
// }

// impl From<&str> for FieldValue {
//     fn from(s: &str) -> Self {
//         FieldValue::String(s.to_string())
//     }
// }

// impl From<bool> for FieldValue {
//     fn from(b: bool) -> Self {
//         FieldValue::Bool(b)
//     }
// }

// impl FieldValue {
//     pub fn is_empty(&self) -> bool {
//         match self {
//             FieldValue::String(value) => value.is_empty(),
//             _ => false,
//         }
//     }
//     pub fn as_string(&self) -> String {
//         match self {
//             FieldValue::String(value) => value.clone(),
//             _ => "x".to_string(),
//         }
//     }
//     pub fn as_bool(&self) -> bool {
//         match self {
//             FieldValue::Bool(value) => *value,
//             _ => false,
//         }
//     }
// }
