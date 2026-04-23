// Standalone HTTP server that exposes /api/v1 backed by SurrealDB.
// Minimal plumbing (no OIDC / webauthn / sessions), intended for running
// the SvelteKit UI against the real database without lifting the full web
// stack.
//
//   cargo run -p web --bin db_server
//   # then in src/web/ui:
//   GUARDRAIL_API_URL=http://127.0.0.1:4500/api/v1 npm run dev

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use surrealdb::opt::auth::Root;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[path = "../db_api.rs"]
mod db_api;

type AnyErr = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), AnyErr> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("GUARDRAIL_DB_HOST")
        .unwrap_or_else(|_| "ws://localhost:8000".into());
    let user = std::env::var("GUARDRAIL_DB_USER").unwrap_or_else(|_| "root".into());
    let pass = std::env::var("GUARDRAIL_DB_PASS").unwrap_or_else(|_| "root".into());
    let ns = std::env::var("GUARDRAIL_DB_NS").unwrap_or_else(|_| "guardrail".into());
    let db_name = std::env::var("GUARDRAIL_DB_NAME").unwrap_or_else(|_| "guardrail".into());

    let addr: SocketAddr = std::env::var("GUARDRAIL_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4500".into())
        .parse()?;

    let db = surrealdb::engine::any::connect(&host).await?;
    db.signin(Root { username: user, password: pass }).await?;
    db.use_ns(&ns).use_db(&db_name).await?;
    let state = db_api::DbState { db: Arc::new(db) };

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .nest("/api/v1", db_api::router().with_state(state))
        .layer(cors);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("db_server listening on http://{addr}/api/v1");
    axum::serve(listener, app).await?;
    Ok(())
}
