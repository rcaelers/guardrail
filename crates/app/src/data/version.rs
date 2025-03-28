use leptos::prelude::*;
use paste::paste;
use uuid::Uuid;

use crate::data_server_functions;
use repos::{
    QueryParams,
    version::{NewVersion, Version},
};

#[cfg(feature = "ssr")]
use repos::version::VersionRepo;

data_server_functions! {Version, "versions",}
