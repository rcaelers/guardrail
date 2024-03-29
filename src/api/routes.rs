use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use jwt_authorizer::{Authorizer, JwtAuthorizer};

use super::minidump::MinidumpApi;
use super::symbols::SymbolsApi;
use crate::api::base::Api;
use crate::app_state::AppState;
use crate::auth::user::AuthenticatedUser;
use crate::entity::prelude;
use crate::settings;

pub async fn routes() -> Router<Arc<AppState>> {
    let url = settings().auth.jwks_url.as_str();
    let auth: Authorizer<AuthenticatedUser> =
        JwtAuthorizer::from_jwks_url(url).build().await.unwrap();

    routes_api()
        .await
        //.layer(auth.into_layer())
        .route("/minidump/upload", post(MinidumpApi::upload))
}

#[cfg(test)]
pub async fn routes_test() -> Router<Arc<AppState>> {
    routes_api()
        .await
        .route("/minidump/upload", post(MinidumpApi::upload))
}

async fn routes_api() -> Router<Arc<AppState>> {
    Router::new()
        // Annotation
        .route("/annotation", post(Api::create::<prelude::Annotation>))
        .route("/annotation", get(Api::get_all::<prelude::Annotation>))
        .route(
            "/annotation/:id",
            get(Api::get_by_id::<prelude::Annotation>),
        )
        .route(
            "/annotation/:id",
            delete(Api::remove_by_id::<prelude::Annotation>),
        )
        .route("/annotation/:id", put(Api::update::<prelude::Annotation>))
        // Attachment
        .route("/attachment", post(Api::create::<prelude::Attachment>))
        .route("/attachment", get(Api::get_all::<prelude::Attachment>))
        .route(
            "/attachment/:id",
            get(Api::get_by_id::<prelude::Attachment>),
        )
        .route(
            "/attachment/:id",
            delete(Api::remove_by_id::<prelude::Attachment>),
        )
        .route("/attachment/:id", put(Api::update::<prelude::Attachment>))
        // Crash
        .route("/crash", post(Api::create::<prelude::Crash>))
        .route("/crash", get(Api::get_all::<prelude::Crash>))
        .route("/crash/:id", get(Api::get_by_id::<prelude::Crash>))
        .route("/crash/:id", delete(Api::remove_by_id::<prelude::Crash>))
        .route("/crash/:id", put(Api::update::<prelude::Crash>))
        // Product
        .route("/product", post(Api::create::<prelude::Product>))
        .route("/product", get(Api::get_all::<prelude::Product>))
        .route("/product/:id", get(Api::get_by_id::<prelude::Product>))
        .route(
            "/product/:id",
            delete(Api::remove_by_id::<prelude::Product>),
        )
        .route("/product/:id", put(Api::update::<prelude::Product>))
        // Symbols
        .route("/symbols", post(Api::create::<prelude::Symbols>))
        .route("/symbols", get(Api::get_all::<prelude::Symbols>))
        .route("/symbols/:id", get(Api::get_by_id::<prelude::Symbols>))
        .route(
            "/symbols/:id",
            delete(Api::remove_by_id::<prelude::Symbols>),
        )
        .route("/symbols/:id", put(Api::update::<prelude::Symbols>))
        // Version
        .route("/version", post(Api::create::<prelude::Version>))
        .route("/version", get(Api::get_all::<prelude::Version>))
        .route("/version/:id", get(Api::get_by_id::<prelude::Version>))
        .route(
            "/version/:id",
            delete(Api::remove_by_id::<prelude::Version>),
        )
        .route("/version/:id", put(Api::update::<prelude::Version>))
        // Symbols
        .route("/symbols/upload", post(SymbolsApi::upload))
}
