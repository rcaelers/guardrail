use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("general authentication failure")]
    Failure,

    #[error("response error. field: {field}, reason: {reason}")]
    ResponseFieldError { field: String, reason: String },

    #[error("invalid token exchange")]
    InvalidTokenExchange,

    #[error("token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("claim verification error: {0}")]
    ClaimVerificationError(String),

    #[error("token signing error: {0}")]
    TokenSigningError(String),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("already authenticated")]
    AlreadyAuthenticated,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let s = self.to_string();
        debug!("{}", s);
        (StatusCode::BAD_REQUEST, s).into_response()
    }
}
