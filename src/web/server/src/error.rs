use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure: {0}")]
    Failure(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("corrupt session")]
    CorruptSession,
}

impl AppError {
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!("web error: {err}");
        Self::InternalFailure()
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self::Failure(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn corrupt_session() -> Self {
        Self::CorruptSession
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::InternalFailure() => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
            }
            AppError::Failure(message) => (StatusCode::BAD_REQUEST, format!("general failure: {message}")),
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, format!("not found: {message}")),
            AppError::CorruptSession => (StatusCode::BAD_REQUEST, "corrupt session".to_string()),
        };

        (status, message).into_response()
    }
}
