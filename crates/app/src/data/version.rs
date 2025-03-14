use crate::{data_server_functions, data_server_name_functions};
use paste::paste;

use leptos::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use repos::{
    QueryParams,
    version::{NewVersion, Version},
};

#[cfg(feature = "ssr")]
use repos::version::VersionRepo;

data_server_functions! {Version, "versions",}
data_server_name_functions! {Version, "versions",}
