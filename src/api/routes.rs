use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;

use super::minidump::MinidumpHandler;
use super::product::Product;
use super::symbols::SymbolsHandler;
use super::version::Version;
use crate::app_state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Symbols
        .route("/symbols/upload", post(SymbolsHandler::upload))
        // Minidump
        .route("/minidump/upload", post(MinidumpHandler::upload))
        // Product
        .route("/product", post(Product::create))
        .route("/product", get(Product::query))
        .route("/product/:id", get(Product::get_by_id))
        .route("/product/:id", delete(Product::remove_by_id))
        .route("/product/:id", put(Product::update_by_id))
        // Version
        .route("/version", post(Version::create))
        .route("/version", get(Version::query))
        .route("/version/:id", get(Version::get_by_id))
        .route("/version/:id", delete(Version::remove_by_id))
        .route("/version/:id", put(Version::update_by_id))
}
