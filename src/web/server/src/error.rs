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

    #[error("forbidden")]
    Forbidden,
}

impl AppError {
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!("web error: {err}");
        Self::InternalFailure()
    }

    #[allow(dead_code)]
    pub fn failure(message: impl Into<String>) -> Self {
        Self::Failure(message.into())
    }

    #[allow(dead_code)]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    #[allow(dead_code)]
    pub fn corrupt_session() -> Self {
        Self::CorruptSession
    }

    pub fn forbidden() -> Self {
        Self::Forbidden
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::InternalFailure() => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
            }
            AppError::Failure(message) => {
                (StatusCode::BAD_REQUEST, format!("general failure: {message}"))
            }
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, format!("not found: {message}")),
            AppError::CorruptSession => (StatusCode::BAD_REQUEST, "corrupt session".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
        };

        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    async fn response_text(err: AppError) -> (StatusCode, String) {
        let response = err.into_response();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("body should read");
        (status, String::from_utf8(body.to_vec()).expect("body should be utf8"))
    }

    #[tokio::test]
    async fn constructors_map_to_expected_responses() {
        assert_eq!(
            response_text(AppError::internal("boom")).await,
            (StatusCode::INTERNAL_SERVER_ERROR, "internal failure".to_string())
        );
        assert_eq!(
            response_text(AppError::failure("bad input")).await,
            (StatusCode::BAD_REQUEST, "general failure: bad input".to_string())
        );
        assert_eq!(
            response_text(AppError::not_found("thing")).await,
            (StatusCode::NOT_FOUND, "not found: thing".to_string())
        );
        assert_eq!(
            response_text(AppError::corrupt_session()).await,
            (StatusCode::BAD_REQUEST, "corrupt session".to_string())
        );
        assert_eq!(
            response_text(AppError::forbidden()).await,
            (StatusCode::FORBIDDEN, "forbidden".to_string())
        );
    }
}
