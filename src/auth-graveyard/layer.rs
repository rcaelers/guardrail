use axum::{body::Body, http::Request, response::Response, Extension, RequestExt};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tower_sessions::Session;

use super::oidc::{OidcClientTraitArc, UserClaims};
use crate::settings::settings;

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub current_user: Option<UserClaims>,
    session: Session,
}

impl AuthContext {
    pub(super) fn new(session: Session) -> Self {
        Self {
            current_user: None,
            session,
        }
    }
}

#[derive(Clone)]
struct AuthState {
    auth_client: OidcClientTraitArc,
}

#[derive(Clone)]
pub struct AuthLayer {
    state: AuthState,
}

impl AuthLayer {
    pub fn new(auth_client: OidcClientTraitArc) -> Self {
        let state = AuthState { auth_client };
        Self { state }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
    state: AuthState,
}

impl<S> Service<Request<Body>> for AuthService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let state = self.state.clone();
        let inner = self.inner.clone();

        let mut inner = std::mem::replace(&mut self.inner, inner);
        Box::pin(async move {
            let Extension(session): Extension<Session> = request
                .extract_parts()
                .await
                .expect("Session extension missing");
            let user = session.get::<UserClaims>("user").unwrap_or(None);

            match user {
                Some(e) => inner.call(request).await,
                None => Ok(Response::builder()
                    .status(axum::http::StatusCode::FOUND)
                    .header(
                        axum::http::header::LOCATION,
                        settings().server.site.clone() + "/auth/login?next=" + request.uri().path(),
                    )
                    .body(Default::default())
                    .unwrap()),
            }
        })
    }
}
