use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use minidump_processor::ProcessError;
use thiserror::Error;
use tracing::error;
use webauthn_rs::prelude::WebauthnError;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure")]
    Failure(String),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError),

    #[error("failed to process minidump: `{0}`")]
    MinidumpError(#[from] minidump::Error),

    #[error("failed to process minidump: `{0}`")]
    MinidumpProcessError(#[from] ProcessError),

    #[error("Product {0} not found")]
    ProductNotFound(String),

    #[error("Version {1} for product {0} not found")]
    VersionNotFound(String, String),

    #[error("Crash not found")]
    CrashNotFound(),

    #[error("User {0} not found")]
    UserNotFound(String),

    #[error("User {0} already exists")]
    UserAlreadyExists(String),

    #[error("Corrupt session")]
    CorruptSession,

    #[error("Deserialising session failed: {0}")]
    InvalidSessionState(#[from] tower_sessions::session::Error),

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
                (StatusCode::BAD_REQUEST, format!("general failure : {}", err))
            }
            ApiError::RepoError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::MinidumpError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::MinidumpProcessError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::UserNotFound(user) => {
                (StatusCode::BAD_REQUEST, format!("User {} not found", user))
            }
            ApiError::UserAlreadyExists(user) => {
                (StatusCode::BAD_REQUEST, format!("User {} already exists", user))
            }
            ApiError::CorruptSession => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Corrupt Session".to_string())
            }
            ApiError::InvalidSessionState(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid Session State: {}", err))
            }
            ApiError::WebauthnError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Webauthn Error: {}", err))
            }
            ApiError::ProductNotFound(product) => {
                (StatusCode::BAD_REQUEST, format!("Product {} not found", product))
            }
            ApiError::VersionNotFound(product, version) => (
                StatusCode::BAD_REQUEST,
                format!("Version {} of product {} not found", version, product),
            ),
            ApiError::CrashNotFound() => (StatusCode::BAD_REQUEST, "Crash not found".to_string()),
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
