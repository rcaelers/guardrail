use ::chrono::NaiveDateTime;
use leptos_struct_table::*;
use repos::product::Product;
use uuid::Uuid;

use crate::{classes::ClassesPreset, components::datatable::ExtraRowTrait};

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

impl From<Product> for ProductRow {
    fn from(product: Product) -> Self {
        Self {
            id: product.id,
            name: product.name,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

impl ExtraRowTrait for ProductRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}
