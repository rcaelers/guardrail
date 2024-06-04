use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

use super::AuthSession;

#[async_trait]
impl<S> FromRequestParts<S> for AuthSession
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthSession>().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Cannot extract auth session. Is AuthLayer enabled?",
        ))
    }
}
