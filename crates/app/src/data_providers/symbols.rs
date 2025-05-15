use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::prelude::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::vec;
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use crate::auth::AuthenticatedUser;
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct SymbolsRow {
    pub id: Uuid,
    pub product: String,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_location: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
}

#[cfg(feature = "ssr")]
#[derive(FromQueryResult, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Symbols {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_location: String,
    pub product_id: Uuid,
    pub product: String,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Symbols {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub storage_location: String,
    pub product_id: Uuid,
    pub product: String,
}

impl From<Symbols> for SymbolsRow {
    fn from(symbols: Symbols) -> Self {
        Self {
            id: symbols.id,
            os: symbols.os,
            arch: symbols.arch,
            build_id: symbols.build_id,
            module_id: symbols.module_id,
            storage_location: symbols.storage_location,
            created_at: symbols.created_at,
            updated_at: symbols.updated_at,
            product_id: Some(symbols.product_id),
            product: symbols.product,
        }
    }
}

impl ExtraRowTrait for SymbolsRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.build_id.clone()
    }
}
