use leptos::prelude::*;
use paste::paste;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{data_server_functions, data_server_name_functions};
use data::user::{NewUser, User};
use common::QueryParams;

#[cfg(feature = "ssr")]
use repos::user::UserRepo;

data_server_functions! {User, "users",}
data_server_name_functions! {User, "users",}
