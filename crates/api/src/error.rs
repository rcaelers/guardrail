use axum::{
    Json,
    extract::rejection::QueryRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::error;
use webauthn_rs::prelude::WebauthnError;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure: {0}")]
    Failure(String),

    #[error("access denied for product {0}")]
    ProductAccessDenied(String),

    #[error("Product {0} not found")]
    ProductNotFound(String),

    #[error("Version {1} for product {0} not found")]
    VersionNotFound(String, String),

    #[error("User {0} not found")]
    UserNotFound(String),

    #[error("User {0} already exists")]
    UserAlreadyExists(String),

    #[error("parameters rejected: `{0}`")]
    QueryExtractorRejection(#[from] QueryRejection),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError),

    #[error("Corrupt session")]
    CorruptSession,

    #[error("Deserialising session failed: {0}")]
    InvalidSession(#[from] tower_sessions::session::Error),

    #[error("Webauthn error: `{0}`")]
    WebauthnError(#[from] WebauthnError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::InternalFailure() => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
            }
            ApiError::Failure(err) => {
                (StatusCode::BAD_REQUEST, format!("general failure: {err}"))
            }
            ApiError::ProductAccessDenied(product) => {
                (StatusCode::FORBIDDEN, format!("access denied for product {product}"))
            }
            ApiError::ProductNotFound(product) => {
                (StatusCode::BAD_REQUEST, format!("product {product} not found"))
            }
            ApiError::VersionNotFound(product, version) => (
                StatusCode::BAD_REQUEST,
                format!("version {version} of product {product} not found"),
            ),
            ApiError::UserNotFound(user) => {
                (StatusCode::BAD_REQUEST, format!("user {user} not found"))
            }
            ApiError::UserAlreadyExists(user) => {
                (StatusCode::BAD_REQUEST, format!("user {user} already exists"))
            }
            ApiError::QueryExtractorRejection(err) => {
                error!("query extractor rejection: {:?}", err);
                (StatusCode::BAD_REQUEST, err.to_string())
            }
            ApiError::RepoError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::CorruptSession => {
                (StatusCode::INTERNAL_SERVER_ERROR, "corrupt session".to_string())
            }
            ApiError::InvalidSession(err) => {
                error!("invalid session: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "invalid session".to_string())
            }
            ApiError::WebauthnError(err) => {
                error!("webauthn error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("webauthn error: {err}"))
            }
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
