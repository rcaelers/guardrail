use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;

use super::annotation::AnnotationApi;
use super::attachment::AttachmentApi;
use super::crash::CrashApi;
use super::minidump::MinidumpApi;
use super::product::ProductApi;
use super::symbols::SymbolsApi;
use super::user::UserApi;
use super::version::VersionApi;
use crate::api::base::BaseApi;
use crate::app_state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    // TODO: delegate Data API to BaseApi
    Router::new()
        // Annotation
        .route("/annotation", post(AnnotationApi::create))
        .route("/annotation", get(AnnotationApi::query))
        .route("/annotation/:id", get(AnnotationApi::get_by_id))
        .route("/annotation/:id", delete(AnnotationApi::remove_by_id))
        .route("/annotation/:id", put(AnnotationApi::update_by_id))
        // Attachment
        .route("/attachment", post(AttachmentApi::create))
        .route("/attachment", get(AttachmentApi::query))
        .route("/attachment/:id", get(AttachmentApi::get_by_id))
        .route("/attachment/:id", delete(AttachmentApi::remove_by_id))
        .route("/attachment/:id", put(AttachmentApi::update_by_id))
        // Crash
        .route("/crash", post(CrashApi::create))
        .route("/crash", get(CrashApi::query))
        .route("/crash/:id", get(CrashApi::get_by_id))
        .route("/crash/:id", delete(CrashApi::remove_by_id))
        .route("/crash/:id", put(CrashApi::update_by_id))
        // Product
        .route("/product", post(ProductApi::create))
        .route("/product", get(ProductApi::query))
        .route("/product/:id", get(ProductApi::get_by_id))
        .route("/product/:id", delete(ProductApi::remove_by_id))
        .route("/product/:id", put(ProductApi::update_by_id))
        // Symbols
        .route("/symbols", post(SymbolsApi::create))
        .route("/symbols", get(SymbolsApi::query))
        .route("/symbols/:id", get(SymbolsApi::get_by_id))
        .route("/symbols/:id", delete(SymbolsApi::remove_by_id))
        .route("/symbols/:id", put(SymbolsApi::update_by_id))
        // User
        .route("/user", post(UserApi::create))
        .route("/user", get(UserApi::query))
        .route("/user/:id", get(UserApi::get_by_id))
        .route("/user/:id", delete(UserApi::remove_by_id))
        .route("/user/:id", put(UserApi::update_by_id))
        // Version
        .route("/version", post(VersionApi::create))
        .route("/version", get(VersionApi::query))
        .route("/version/:id", get(VersionApi::get_by_id))
        .route("/version/:id", delete(VersionApi::remove_by_id))
        .route("/version/:id", put(VersionApi::update_by_id))
        // Symbols
        .route("/symbols/upload", post(SymbolsApi::upload))
        // Minidump
        .route("/minidump/upload", post(MinidumpApi::upload))
}
