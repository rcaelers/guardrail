use axum::{
    Json,
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use minidump_processor::ProcessError;
use thiserror::Error;
use tracing::{error, warn};
use webauthn_rs::prelude::WebauthnError;

#[derive(Error, Debug)]
pub enum ApiError {
    // Original API errors
    #[error("general failure")]
    Failure,

    #[error("database error: `{0}`")]
    DatabaseError(#[from] sqlx::Error),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError),

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

    // Auth-related errors merged from AuthError
    #[error("Corrupt session")]
    CorruptSession,

    #[error("User not found")]
    UserNotFound,

    #[error("User already exists")]
    UserAlreadyExists,

    #[error("Deserialising session failed: {0}")]
    InvalidSessionState(#[from] tower_sessions::session::Error),

    #[error("Webauthn error: `{0}`")]
    WebauthnError(#[from] WebauthnError),

    // #[error("Invalid login credentials")]
    // InvalidCredentials,
}

// Also define AuthError as a type alias to ApiError for backward compatibility
pub type AuthError = ApiError;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            // Original API error handling
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

            // Auth error handling
            ApiError::UserNotFound => (StatusCode::BAD_REQUEST, "User not found".to_string()),
            ApiError::UserAlreadyExists => (StatusCode::BAD_REQUEST, "User already exists".to_string()),
            ApiError::CorruptSession => (StatusCode::INTERNAL_SERVER_ERROR, "Corrupt Session".to_string()),
            ApiError::InvalidSessionState(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid Session State: {}", err),
            ),
            ApiError::WebauthnError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Webauthn Error: {}", err),
            ),
            // ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()),
        };

        // Different response format for auth errors vs API errors
        if matches!(self,
            ApiError::UserNotFound |
            ApiError::UserAlreadyExists |
            ApiError::CorruptSession |
            ApiError::InvalidSessionState(_) |
            ApiError::WebauthnError(_)
            // ApiError::InvalidCredentials
        ) {
            // Log internal server errors but return generic message to client
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                error!("Internal Server Error: {}", error_message);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            } else {
                warn!("Auth Error: {}", error_message);
                (status, error_message).into_response()
            }
        } else {
            // Original API error response format
            let body = Json(serde_json::json!({
                "result": "failed",
                "error": error_message,
            }));

            (status, body).into_response()
        }
    }
}

fn handle_database_error(err: &sqlx::Error) -> (StatusCode, String) {
    match err {
        sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "record not found".to_string()),
        _ => (StatusCode::BAD_REQUEST, err.to_string()),
    }
}
