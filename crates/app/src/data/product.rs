use crate::{data_server_functions, data_server_name_functions};
use paste::paste;

use leptos::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use repos::{
    QueryParams,
    product::{NewProduct, Product},
};

#[cfg(feature = "ssr")]
use repos::product::ProductRepo;

data_server_functions! {Product, "products",}
data_server_name_functions! {Product, "products",}

