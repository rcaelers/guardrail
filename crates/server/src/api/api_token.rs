use axum::{
    extract::Request,
    http::{StatusCode, header},
    response::Response,
};
use futures::future::BoxFuture;
use repos::api_token::ApiTokenRepo;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::{error, info};

use crate::app_state::AppState;

use super::verify_token;

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
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body("Unauthorized: Missing API token".into())
                        .unwrap());
                }
            };

            let mut conn = match app_state.repo.acquire_admin().await {
                Ok(conn) => conn,
                Err(err) => {
                    error!("Failed to get database connection: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body("Internal server error".into())
                        .unwrap());
                }
            };

            let tokens = match ApiTokenRepo::get_all(&mut *conn).await {
                Ok(tokens) => tokens
                    .into_iter()
                    .filter(|t| t.is_active)
                    .collect::<Vec<_>>(),
                Err(err) => {
                    error!("Database error when retrieving tokens: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body("Internal server error".into())
                        .unwrap());
                }
            };

            let mut valid_token = None;
            for token in tokens {
                match verify_token(&token_str, &token.token_hash) {
                    Ok(true) => {
                        valid_token = Some(token);
                        break;
                    }
                    Ok(false) => continue,
                    Err(err) => {
                        error!("Error verifying token: {}", err);
                        continue;
                    }
                }
            }

            let api_token = match valid_token {
                Some(token) => token,
                None => {
                    return Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body("Unauthorized: Invalid API token".into())
                        .unwrap());
                }
            };

            if !ApiTokenRepo::has_entitlement(&api_token, required_entitlement.as_str()) {
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body("Forbidden: Insufficient permissions".into())
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

