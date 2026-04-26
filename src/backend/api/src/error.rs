use axum::{
    Json,
    extract::rejection::QueryRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure: {0}")]
    Failure(String),

    #[error("{0}")]
    InvalidToken(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("access denied for product {0}")]
    ProductAccessDenied(String),

    #[error("Product {0} not found")]
    ProductNotFound(String),

    #[error("Product {0} not accepting crashes")]
    ProductNotAcceptingCrashes(String),

    #[error("Upload validation for product {0} failed: {1}")]
    ValidationError(String, String),

    #[error("User {0} not found")]
    UserNotFound(String),

    #[error("User {0} already exists")]
    UserAlreadyExists(String),

    #[error("parameters rejected: `{0}`")]
    QueryExtractorRejection(#[from] QueryRejection),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::InternalFailure() => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
            }
            ApiError::Failure(err) => (StatusCode::BAD_REQUEST, format!("general failure: {err}")),
            ApiError::InvalidToken(msg) => (StatusCode::UNAUTHORIZED, msg.to_string()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.to_string()),
            ApiError::ProductAccessDenied(product) => {
                (StatusCode::FORBIDDEN, format!("access denied for product {product}"))
            }
            ApiError::ProductNotFound(product) => {
                (StatusCode::BAD_REQUEST, format!("product {product} not found"))
            }
            ApiError::ProductNotAcceptingCrashes(product) => {
                (StatusCode::BAD_REQUEST, format!("product {product} not accepting crashes"))
            }
            ApiError::ValidationError(product, error_message) => (
                StatusCode::BAD_REQUEST,
                format!("validation of product {product} failed: {error_message}"),
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
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
