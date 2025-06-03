use ::chrono::NaiveDateTime;
use data::crash::Crash;
use leptos::prelude::*;
use leptos_struct_table::*;
use std::fmt::Debug;
use uuid::Uuid;

use crate::{classes::ClassesPreset, components::datatable::ExtraRowTrait};

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct CrashRow {
    pub id: Uuid,
    pub product: String,
    pub version: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
}

impl From<Crash> for CrashRow {
    fn from(crash: Crash) -> Self {
        Self {
            id: crash.id,
            created_at: crash.created_at,
            updated_at: crash.updated_at,
            product_id: Some(crash.product_id),
            product: "TODO:".to_string(), // crash.product,
            version: "TODO:".to_string(), // crash.version,
        }
    }
}

impl ExtraRowTrait for CrashRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.product.clone()
    }
}
