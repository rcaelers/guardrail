use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;
use thiserror::Error;
use tracing::{error, warn};
use webauthn_rs::prelude::WebauthnError;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Corrupt session")]
    CorruptSession,
    #[error("User not found")]
    UserNotFound,
    #[error("User already exists")]
    UserAlreadyExists,
    // #[error("User has no credentials")]
    // UserHasNoCredentials,
    #[error("Deserialising session failed: {0}")]
    InvalidSessionState(#[from] tower_sessions::session::Error),
    #[error("Error during serialisation/deserialisation: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Database error: `{0}`")]
    DatabaseError(#[from] DbErr),
    #[error("Webauthn error: `{0}`")]
    WebauthnError(#[from] WebauthnError),
}
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AuthError::UserNotFound => (StatusCode::BAD_REQUEST, "User not found".to_string()),
            AuthError::UserAlreadyExists => {
                (StatusCode::BAD_REQUEST, "User already exists".to_string())
            }
            // AuthError::UserHasNoCredentials => (
            //     StatusCode::BAD_REQUEST,
            //     "User has no credentials".to_string(),
            // ),
            AuthError::CorruptSession => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Corrupt Session".to_string(),
            ),
            AuthError::InvalidSessionState(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid Session State: {}", err),
            ),
            AuthError::SerializationError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Serialization Error: {}", err),
            ),
            AuthError::DatabaseError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database Error: {}", err),
            ),
            AuthError::WebauthnError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Webauthn Error: {}", err),
            ),
        };

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            error!("Internal Server Error: {}", body);
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        } else {
            warn!("Bad Request: {}", body);
            (status, body).into_response()
        }
    }
}
