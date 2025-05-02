use ::chrono::NaiveDateTime;
use leptos::prelude::*;
use leptos_struct_table::*;
use data::user::User;
use uuid::Uuid;

use crate::{classes::ClassesPreset, components::datatable::ExtraRowTrait};

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

impl From<User> for UserRow {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            is_admin: user.is_admin,
            username: user.username,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl ExtraRowTrait for UserRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.username.clone()
    }
}
