use axum::{
    Json,
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use minidump_processor::ProcessError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("general failure")]
    Failure,

    #[error("database error: `{0}`")]
    DatabaseError(#[from] sqlx::Error),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError), // TODO: extend

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
        let s = self.to_string();
        print!("{}", s);
        let (status, error_message) = match self {
            ApiError::Failure => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "general failure".to_owned(),
            ),
            ApiError::DatabaseError(err) => handle_database_error(err),
            ApiError::RepoError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::MinidumpError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::IOError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::MultiPartError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::JoinError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::JsonError(err) => (StatusCode::BAD_REQUEST, format!("invalid JSON: {}", err)),
            ApiError::MinidumpProcessError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

fn handle_database_error(err: sqlx::Error) -> (StatusCode, String) {
    match err {
        sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, err.to_string()),
        _ => (StatusCode::BAD_REQUEST, err.to_string()),
    }
}
