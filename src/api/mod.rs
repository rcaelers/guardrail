mod annotation;
mod attachment;
mod base;
mod crash;
mod error;
mod minidump;
mod product;
mod routes;
mod symbols;
mod version;
pub use routes::routes;
use serde::Deserialize;

// TODO: Merge with oidc user
#[derive(Debug, Deserialize, Clone)]
pub struct User {
    sub: String,
}
