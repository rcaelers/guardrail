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

    #[error("invalid token: {0}")]
    InvalidToken(String),

    #[error("access denied for product {0}")]
    ProductAccessDenied(String),

    #[error("Product {0} not found")]
    ProductNotFound(String),

    #[error("Product {0} not accepting crashes")]
    ProductNotAcceptingCrashes(String),

    #[error("Upload validation for product {0} failed: {1}")]
    ValidationError(String, String),

    #[error("parameters rejected: `{0}`")]
    QueryExtractorRejection(#[from] QueryRejection),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::InternalFailure() => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
            }
            ApiError::Failure(err) => (StatusCode::BAD_REQUEST, format!("general failure: {err}")),
            ApiError::InvalidToken(token) => {
                (StatusCode::FORBIDDEN, format!("invalid token: {token}"))
            }
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
            ApiError::QueryExtractorRejection(err) => {
                error!("query extractor rejection: {:?}", err);
                (StatusCode::BAD_REQUEST, err.to_string())
            }
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
