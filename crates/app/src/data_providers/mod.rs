pub mod product;
pub mod user;
pub mod version;
pub mod symbols;

use leptos::*;
//use leptos_struct_table::*;
use uuid::Uuid;

pub trait ExtraTableDataProvider<T> {
    // fn new(parent_id: Option<Uuid>) -> TableDataProvider<T>;
    fn update(&self);
    fn get_filter_signal(&self) -> RwSignal<String>;
}

pub trait ExtraRowTrait {
    fn get_id(&self) -> Uuid;
    fn get_name(&self) -> String;
}
