pub mod product;
pub mod symbols;
pub mod user;
pub mod version;

use leptos::*;
use uuid::Uuid;

pub trait ExtraTableDataProvider<T> {
    fn update(&self);
    fn get_filter_signal(&self) -> RwSignal<String>;
}

pub trait ExtraRowTrait {
    fn get_id(&self) -> Uuid;
    fn get_name(&self) -> String;
}
