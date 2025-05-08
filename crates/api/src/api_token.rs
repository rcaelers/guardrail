use axum::{
    Json,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use common::token::{decode_api_token, verify_api_secret};
use futures::future::BoxFuture;
use repos::api_token::ApiTokenRepo;
use serde_json::json;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::{error, info};

use crate::state::AppState;

#[derive(Clone)]
pub enum RequiredEntitlement {
    SymbolUpload,
    MinidumpUpload,
    Token,
}

impl RequiredEntitlement {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequiredEntitlement::SymbolUpload => "symbol-upload",
            RequiredEntitlement::MinidumpUpload => "minidump-upload",
            RequiredEntitlement::Token => "token",
        }
    }
}

fn extract_api_token<B>(request: &Request<B>) -> Option<String> {
    let auth_header = request.headers().get("Authorization")?;
    let auth_value = auth_header.to_str().ok()?;

    if let Some(token) = auth_value.strip_prefix("Bearer ") {
        Some(token.to_string())
    } else if let Some(token) = auth_value.strip_prefix("Token ") {
        Some(token.to_string())
    } else {
        Some(auth_value.to_string())
    }
}

#[derive(Clone)]
pub struct ApiTokenLayer {
    app_state: AppState,
    required_entitlement: RequiredEntitlement,
}

impl ApiTokenLayer {
    pub fn new(app_state: AppState, required_entitlement: RequiredEntitlement) -> Self {
        Self {
            app_state,
            required_entitlement,
        }
    }
}

impl<S> Layer<S> for ApiTokenLayer {
    type Service = ApiTokenService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApiTokenService {
            inner,
            app_state: self.app_state.clone(),
            required_entitlement: self.required_entitlement.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ApiTokenService<S> {
    inner: S,
    app_state: AppState,
    required_entitlement: RequiredEntitlement,
}

impl<S, B> Service<Request<B>> for ApiTokenService<S>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let required_entitlement = self.required_entitlement.clone();
        let app_state = self.app_state.clone();

        Box::pin(async move {
            let token_str = match extract_api_token(&request) {
                Some(token) => token,
                None => {
                    return Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "missing API token"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
            };

            let (token_id, token_secret) = match decode_api_token(&token_str) {
                Ok((id, secret)) => (id, secret),
                Err(err) => {
                    error!("Failed to get decode api key {}: {}", token_str, err);
                    return Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "invalid API token"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
            };

            let mut conn = match app_state.repo.acquire_admin().await {
                Ok(conn) => conn,
                Err(err) => {
                    error!("Failed to get database connection: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "internal server error"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
            };

            let api_token = match ApiTokenRepo::get_by_token_id(&mut *conn, token_id).await {
                Ok(Some(api_token)) => api_token,
                Ok(None) => {
                    return Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "invalid API token"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
                Err(err) => {
                    error!("Database error when retrieving api token: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "internal server error"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
            };

            let token_status = match verify_api_secret(&token_secret, &api_token.token_hash) {
                Ok(true) => true,
                Ok(false) => false,
                Err(err) => {
                    error!("Failed to verify API token: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            Json(json!({
                                "result": "failed",
                                "error": "invalid API token"
                            }))
                            .into_response()
                            .into_body(),
                        )
                        .unwrap());
                }
            };

            if !token_status {
                return Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(
                        Json(json!({
                            "result": "failed",
                            "error": "invalid API token"
                        }))
                        .into_response()
                        .into_body(),
                    )
                    .unwrap());
            }

            if !api_token.is_valid() {
                return Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(
                        Json(json!({
                            "result": "failed",
                            "error": "API token is expired or inactive"
                        }))
                        .into_response()
                        .into_body(),
                    )
                    .unwrap());
            }

            if !api_token.has_entitlement(required_entitlement.as_str()) {
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(
                        Json(json!({
                            "result": "failed",
                            "error": "insufficient permissions"
                        }))
                        .into_response()
                        .into_body(),
                    )
                    .unwrap());
            }

            if let Err(err) = ApiTokenRepo::update_last_used(&mut *conn, api_token.id).await {
                error!("Failed to update last_used_at: {}", err);
            }

            if let Some(product_id) = api_token.product_id {
                if let Some(user_id) = api_token.user_id {
                    info!(
                        "API token validated successfully - product_id: {}, user_id: {}, token_id: {}",
                        product_id, user_id, api_token.id
                    );
                } else {
                    info!(
                        "API token validated successfully - product_id: {}, token_id: {}",
                        product_id, api_token.id
                    );
                }
            } else if let Some(user_id) = api_token.user_id {
                info!(
                    "API token validated successfully - user_id: {}, token_id: {}",
                    user_id, api_token.id
                );
            } else {
                info!("API token validated successfully - token_id: {}", api_token.id);
            }

            request.extensions_mut().insert(api_token);
            inner.call(request).await
        })
    }
}
