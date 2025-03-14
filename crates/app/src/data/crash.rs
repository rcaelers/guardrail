use crate::data_server_functions;
use paste::paste;

use leptos::prelude::*;
use repos::QueryParams;
use repos::crash::{Crash, NewCrash};
use uuid::Uuid;

#[cfg(feature = "ssr")]
use repos::crash::CrashRepo;

data_server_functions! {Crash, "crashes",}
