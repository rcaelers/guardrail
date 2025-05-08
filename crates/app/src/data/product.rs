use leptos::prelude::*;
use paste::paste;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{data_server_functions, data_server_name_functions};
use common::QueryParams;
use data::product::{NewProduct, Product};

#[cfg(feature = "ssr")]
use repos::product::ProductRepo;

data_server_functions! {Product, "products",}
data_server_name_functions! {Product, "products",}
