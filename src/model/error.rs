use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::DbErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("general data failure")]
    Failure(#[from] DbErr),

    #[error("record not found: `{0}`")]
    RecordNotFound(String),
}

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            DbError::RecordNotFound(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database failure: {}", err),
            ),
            DbError::Failure(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database failure: {}", err),
            ),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
