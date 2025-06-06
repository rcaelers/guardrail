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
    pub version: String,
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
    #[table(skip)]
    pub version_id: Option<Uuid>,
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
    pub file_location: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
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
    pub file_location: String,
    pub product_id: Uuid,
    pub version_id: Uuid,
    pub product: String,
    pub version: String,
}

impl From<Symbols> for SymbolsRow {
    fn from(symbols: Symbols) -> Self {
        Self {
            id: symbols.id,
            os: symbols.os,
            arch: symbols.arch,
            build_id: symbols.build_id,
            module_id: symbols.module_id,
            file_location: symbols.file_location,
            created_at: symbols.created_at,
            updated_at: symbols.updated_at,
            product_id: Some(symbols.product_id),
            version_id: Some(symbols.version_id),
            product: symbols.product,
            version: symbols.version,
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

#[server]
pub async fn symbols_get(id: Uuid) -> Result<Symbols, ServerFnError> {
}

#[server]
pub async fn symbols_list(
    #[server(default)] parents: HashMap<String, Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Symbols>, ServerFnError> {
}

#[server]
pub async fn symbols_list_names(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<HashSet<String>, ServerFnError> {
}

#[server]
pub async fn symbols_add(symbols: Symbols) -> Result<(), ServerFnError> {
}

#[server]
pub async fn symbols_update(symbols: Symbols) -> Result<(), ServerFnError> {
}

#[server]
pub async fn symbols_remove(id: Uuid) -> Result<(), ServerFnError> {
}

#[server]
pub async fn symbols_count(
    #[server(default)] parents: HashMap<String, Uuid>,
) -> Result<usize, ServerFnError> {
}
