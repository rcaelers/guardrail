use axum::{
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use minidump_processor::ProcessError;
use sea_orm::DbErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("general failure")]
    Failure,

    #[error("API failure")]
    APIFailure(String),

    #[error("{0} not found with ID '{1}'")]
    ForeignKeyError(String, String),

    #[error("database error: `{0}`")]
    DatabaseError(#[from] DbErr),

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
            ApiError::MinidumpError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::IOError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::MultiPartError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::JoinError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::JsonError(err) => (StatusCode::BAD_REQUEST, format!("invalid JSON: {}", err)),
            ApiError::MinidumpProcessError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::APIFailure(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::ForeignKeyError(r, k) => (StatusCode::NOT_FOUND, s),
        };

        let body = Json(serde_json::json!({
            "result": "failed",
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

fn handle_database_error(err: DbErr) -> (StatusCode, String) {
    match err {
        DbErr::RecordNotFound(e) => (StatusCode::NOT_FOUND, e.to_string()),
        _ => (StatusCode::BAD_REQUEST, err.to_string()),
    }
}
