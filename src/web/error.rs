use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum WebError {
    #[error("general failure")]
    Failure,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let s = self.to_string();
        debug!("{}", s);
        (StatusCode::BAD_REQUEST, s).into_response()
    }
}
