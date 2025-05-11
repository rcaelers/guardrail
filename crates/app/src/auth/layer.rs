use crate::auth::{AuthSession, AuthenticatedUser};
use axum::{Extension, RequestExt, body::Body, http::Request, response::Response};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tower_sessions::Session;

#[derive(Clone)]
struct AuthState {}

#[derive(Clone)]
pub struct AuthLayer {
    state: AuthState,
}

impl AuthLayer {
    pub fn new() -> Self {
        let state = AuthState {};
        Self { state }
    }
}

impl Default for AuthLayer {
    fn default() -> Self {
        Self::new()
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
        let _state = self.state.clone();
        let inner = self.inner.clone();

        let mut inner = std::mem::replace(&mut self.inner, inner);
        Box::pin(async move {
            let Extension(session): Extension<Session> = request
                .extract_parts()
                .await
                .expect("Session extension missing");

            let user = session
                .get::<AuthenticatedUser>("authenticated_user")
                .await
                .unwrap_or(None);

            let auth_session = AuthSession::new(session, user.clone());
            request.extensions_mut().insert(auth_session);
            inner.call(request).await
        })
    }
}
