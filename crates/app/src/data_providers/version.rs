use ::chrono::NaiveDateTime;
use leptos_struct_table::*;
use repos::version::Version;
use uuid::Uuid;

use crate::{classes::ClassesPreset, components::datatable::ExtraRowTrait};

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct VersionRow {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
}

impl From<Version> for VersionRow {
    fn from(version: Version) -> Self {
        Self {
            id: version.id,
            name: version.name,
            hash: version.hash,
            tag: version.tag,
            product_id: Some(version.product_id),
            created_at: version.created_at,
            updated_at: version.updated_at,
            product: "TODO:".to_string(), // version.product,
        }
    }
}

impl ExtraRowTrait for VersionRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}
