// Standalone mock REST server. Runs the same /api/v1 router that
// `web` mounts, but without SurrealDB, OIDC, WebAuthn, or the rest of the
// production stack. Intended for local UI development:
//
//   cargo run -p web --bin mock_server                         # 127.0.0.1:4500
//   GUARDRAIL_MOCK_ADDR=0.0.0.0:7000 cargo run -p web --bin mock_server
//
// Then point the SvelteKit dev server at it:
//
//   GUARDRAIL_API_URL=http://127.0.0.1:4500/api/v1 npm run dev

use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[path = "../mock_api.rs"]
mod mock_api;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = std::env::var("GUARDRAIL_MOCK_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4500".to_string())
        .parse()
        .expect("invalid GUARDRAIL_MOCK_ADDR");

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let app = Router::new()
        .nest(
            "/api/v1",
            mock_api::router().with_state(mock_api::MockState::new()),
        )
        .layer(cors);

    let listener = TcpListener::bind(addr).await.expect("bind failed");
    eprintln!("mock_server listening on http://{addr}/api/v1");
    axum::serve(listener, app).await.expect("serve failed");
}
