// Standalone HTTP server that exposes /api/v1 backed by SurrealDB.
// Minimal plumbing (no OIDC / webauthn / sessions), intended for running
// the SvelteKit UI against the real database without lifting the full web
// stack.
//
//   cargo run -p web --bin db_server
//   # then in src/web/ui:
//   GUARDRAIL_API_URL=http://127.0.0.1:4500/api/v1 npm run dev

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use common::settings::Settings;
use repos::Repo;
use surrealdb::opt::auth::Root;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[path = "../jwt.rs"]
mod jwt;

#[path = "../db_api.rs"]
mod db_api;

type AnyErr = Box<dyn std::error::Error + Send + Sync>;

fn default_config_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../config")
        .to_string_lossy()
        .into_owned()
}

#[tokio::main]
async fn main() -> Result<(), AnyErr> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("GUARDRAIL_DB_HOST").unwrap_or_else(|_| "ws://localhost:8000".into());
    let user = std::env::var("GUARDRAIL_DB_USER").unwrap_or_else(|_| "root".into());
    let pass = std::env::var("GUARDRAIL_DB_PASS").unwrap_or_else(|_| "root".into());
    let ns = std::env::var("GUARDRAIL_DB_NS").unwrap_or_else(|_| "guardrail".into());
    let db_name = std::env::var("GUARDRAIL_DB_NAME").unwrap_or_else(|_| "guardrail".into());
    let config_dir = std::env::var("GUARDRAIL_CONFIG_DIR").unwrap_or_else(|_| default_config_dir());

    let addr: SocketAddr = std::env::var("GUARDRAIL_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4500".into())
        .parse()?;

    let settings = Arc::new(Settings::with_config_dir(&config_dir)?);
    let db = surrealdb::engine::any::connect(&host).await?;
    db.signin(Root {
        username: user,
        password: pass,
    })
    .await?;
    db.use_ns(&ns).use_db(&db_name).await?;

    // Register the JWT access method so RLS $auth variables are populated.
    {
        let public_key = &settings.auth.jwk.public_key;
        db.query(format!(
            r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                WITH JWT ALGORITHM EDDSA KEY '{public_key}'
                DURATION FOR SESSION 1h"#
        ))
        .await?;
    }

    let storage = common::init_s3_object_store(settings.clone()).await;
    let state = db_api::DbState {
        repo: Repo::new(db),
        storage,
        settings,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let app = Router::new()
        .nest("/api/v1", db_api::router().with_state(state))
        .layer(cors);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("db_server listening on http://{addr}/api/v1");
    axum::serve(listener, app).await?;
    Ok(())
}
