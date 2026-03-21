use leptos::prelude::*;
use paste::paste;
use uuid::Uuid;

use crate::data_server_functions;
use common::QueryParams;
use data::crash::{Crash, NewCrash};

#[cfg(feature = "ssr")]
use repos::crash::CrashRepo;

data_server_functions! {Crash, "crashes",}
