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
            ApiError::RepoError(repos::error::RepoError::ConnectionError()) => {
                (StatusCode::SERVICE_UNAVAILABLE, "database unavailable".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_parts(error: ApiError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn maps_errors_to_expected_responses() {
        let cases = [
            (
                ApiError::InternalFailure(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal failure",
            ),
            (
                ApiError::Failure("bad".to_string()),
                StatusCode::BAD_REQUEST,
                "general failure: bad",
            ),
            (
                ApiError::InvalidToken("missing".to_string()),
                StatusCode::UNAUTHORIZED,
                "missing",
            ),
            (ApiError::Forbidden("no".to_string()), StatusCode::FORBIDDEN, "no"),
            (
                ApiError::ProductAccessDenied("product".to_string()),
                StatusCode::FORBIDDEN,
                "access denied for product product",
            ),
            (
                ApiError::ProductNotFound("product".to_string()),
                StatusCode::BAD_REQUEST,
                "product product not found",
            ),
            (
                ApiError::ProductNotAcceptingCrashes("product".to_string()),
                StatusCode::BAD_REQUEST,
                "product product not accepting crashes",
            ),
            (
                ApiError::ValidationError("product".to_string(), "bad".to_string()),
                StatusCode::BAD_REQUEST,
                "validation of product product failed: bad",
            ),
            (
                ApiError::UserNotFound("user".to_string()),
                StatusCode::BAD_REQUEST,
                "user user not found",
            ),
            (
                ApiError::UserAlreadyExists("user".to_string()),
                StatusCode::BAD_REQUEST,
                "user user already exists",
            ),
            (
                ApiError::RepoError(repos::error::RepoError::ConnectionError()),
                StatusCode::SERVICE_UNAVAILABLE,
                "database unavailable",
            ),
        ];

        for (error, expected_status, expected_message) in cases {
            let (status, body) = response_parts(error).await;
            assert_eq!(status, expected_status);
            assert_eq!(body["result"], "failed");
            assert_eq!(body["error"], expected_message);
        }
    }

    #[tokio::test]
    async fn maps_query_rejection_to_bad_request() {
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct Params {
            value: u32,
        }

        let rejection =
            axum::extract::Query::<Params>::try_from_uri(&"/test?value=abc".parse().unwrap())
                .unwrap_err();
        let (status, body) = response_parts(ApiError::from(rejection)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["result"], "failed");
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("Failed to deserialize")
        );
    }
}
