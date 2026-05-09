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

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

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
            ApiError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};

    async fn response_parts(err: ApiError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }

    #[tokio::test]
    async fn maps_errors_to_expected_responses() {
        let cases = [
            (ApiError::InternalFailure(), StatusCode::INTERNAL_SERVER_ERROR, "internal failure"),
            (
                ApiError::Failure("bad input".to_string()),
                StatusCode::BAD_REQUEST,
                "general failure: bad input",
            ),
            (
                ApiError::ServiceUnavailable("cache down".to_string()),
                StatusCode::SERVICE_UNAVAILABLE,
                "cache down",
            ),
            (
                ApiError::InvalidToken("abc".to_string()),
                StatusCode::FORBIDDEN,
                "invalid token: abc",
            ),
            (
                ApiError::ProductAccessDenied("prod".to_string()),
                StatusCode::FORBIDDEN,
                "access denied for product prod",
            ),
            (
                ApiError::ProductNotFound("prod".to_string()),
                StatusCode::BAD_REQUEST,
                "product prod not found",
            ),
            (
                ApiError::ProductNotAcceptingCrashes("prod".to_string()),
                StatusCode::BAD_REQUEST,
                "product prod not accepting crashes",
            ),
            (
                ApiError::ValidationError("prod".to_string(), "nope".to_string()),
                StatusCode::BAD_REQUEST,
                "validation of product prod failed: nope",
            ),
        ];

        for (err, expected_status, expected_error) in cases {
            let (status, body) = response_parts(err).await;
            assert_eq!(status, expected_status);
            assert_eq!(body["result"], "failed");
            assert_eq!(body["error"], expected_error);
        }
    }

    #[tokio::test]
    async fn maps_query_rejection_to_bad_request() {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct Params {
            count: u32,
        }

        let request = axum::http::Request::builder()
            .uri("/?count=not-a-number")
            .body(Body::empty())
            .unwrap();
        let rejection = axum::extract::Query::<Params>::try_from_uri(request.uri()).unwrap_err();
        let (status, body) = response_parts(ApiError::from(rejection)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["result"], "failed");
        assert!(body["error"].as_str().unwrap().contains("count"));
    }
}
