use axum::Router;
use common::settings::settings;
use jwt_authorizer::{Authorizer, IntoLayer, JwtAuthorizer, RegisteredClaims, Validation};

//use super::{minidump::MinidumpApi, symbols::SymbolsApi};
use crate::app_state::AppState;

pub async fn routes() -> Router<AppState> {
    let validation = Validation::new().aud(&["Guardrail"]).leeway(20);

    let auth: Authorizer<RegisteredClaims> =
        JwtAuthorizer::from_ed_pem(settings().auth.jwk.key.as_str())
            .validation(validation)
            .build()
            .await
            .unwrap();

    Router::new()
        //.route("/minidump/upload", post(MinidumpApi::upload))
        //.layer(auth.into_layer())
}

#[cfg(test)]
pub async fn routes_test() -> Router<AppState> {
    Router::new()
    //    .route("/minidump/upload", post(MinidumpApi::upload))
}
