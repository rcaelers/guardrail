use crate::model::error::DbError;

use axum::{
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use minidump_processor::ProcessError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("general failure")]
    Failure,

    #[error("database access error `{0}`")]
    DatabaseError(#[from] DbError),

    #[error("failed to process minidump: `{0}`")]
    MinidumpError(#[from] minidump::Error),

    #[error("failed to process minidump: `{0}`")]
    MinidumpProcessError(#[from] ProcessError),

    #[error("io-error: `{0}`")]
    IOError(#[from] std::io::Error),

    #[error("json error: `{0}`")]
    JsonError(#[from] serde_json::Error),

    #[error("failed to process multipart request: `{0}`")]
    MultiPartError(#[from] MultipartError),

    #[error("thread: `{0}`")]
    JoinError(#[from] tokio::task::JoinError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::Failure => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "general failure".to_owned(),
            ),
            ApiError::DatabaseError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::MinidumpError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::IOError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::MultiPartError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::JoinError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::JsonError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::MinidumpProcessError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
