mod api;
mod app_state;
mod pg_session_store;

use api::{generate_token, hash_token};
use app::auth::AuthSession;
use app::auth::layer::AuthLayer;
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum_server::tls_rustls::RustlsConfig;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list, handle_server_fns_with_context};
use repos::Repo;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use time::Duration;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::level_filters::LevelFilter;
use tracing::{Level, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, FmtSubscriber, fmt};
use webauthn_rs::prelude::*;

use app::*;
use app_state::AppState;
use common::settings::settings;
use pg_session_store::PostgresStore;

async fn init_logging() {
    let directory = &settings().logger.directory;

    let file_appender = tracing_appender::rolling::never(directory, "guardrail.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let max_level = settings().logger.level.parse().unwrap_or(Level::DEBUG);

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap()
        .add_directive("server=debug".parse().unwrap())
        .add_directive("leptos=debug".parse().unwrap())
        .add_directive("app=debug".parse().unwrap());

    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(filter)
        .finish()
        .with(fmt::Layer::new().with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

fn create_webauthn() -> Arc<Webauthn> {
    let rp_id = settings().auth.id.as_str();
    let rp_origin = Url::parse(settings().auth.origin.as_str()).expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let builder = builder.rp_name(settings().auth.name.as_str());

    Arc::new(builder.build().expect("Invalid configuration"))
}

async fn server_fn_handler(
    auth_session: AuthSession,
    State(app_state): State<AppState>,
    request: Request<Body>,
) -> impl IntoResponse {
    handle_server_fns_with_context(
        move || {
            provide_context(app_state.repo.clone());
            provide_context(auth_session.clone());
            provide_context(auth_session.user.clone());
        },
        request,
    )
    .await
}

async fn leptos_routes_handler(
    auth_session: AuthSession,
    state: State<AppState>,
    req: Request<Body>,
) -> Response {
    let State(app_state) = state.clone();
    let handler = leptos_axum::render_route_with_context(
        app_state.routes.clone(),
        move || {
            provide_context(app_state.repo.clone());
            provide_context(auth_session.clone());
            provide_context(auth_session.user.clone());
        },
        move || shell(app_state.leptos_options.clone()),
    );
    handler(state, req).await.into_response()
}

async fn init_db() -> Result<PgPool, sqlx::Error> {
    let database_url = &settings().database.uri;
    let mut opts: PgConnectOptions = database_url.parse()?;
    opts = opts.log_statements(log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

async fn ensure_default_api_token(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    use repos::api_token::{ApiTokenRepo, NewApiToken};
    use tracing::info;

    let mut conn = repo.acquire_admin().await?;

    let tokens = ApiTokenRepo::get_all(&mut *conn).await?;
    if !tokens.is_empty() {
        info!("API tokens already exist, skipping default token creation");
        return Ok(());
    }

    let token = generate_token();
    let token_hash = hash_token(&token).map_err(|_| "Failed to hash token")?;

    let new_token = NewApiToken {
        description: "Default API token".to_string(),
        token_hash,
        product_id: None,
        user_id: None,
        entitlements: vec!["token".to_string()],
        expires_at: None,
    };

    let _token_id = ApiTokenRepo::create(&mut *conn, new_token).await?;
    info!("Created default API token: {}", token);

    Ok(())
}

#[tokio::main]
async fn main() {
    init_logging().await;

    info!("Starting server on port {}", settings().server.port);

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let _addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let db = init_db().await.unwrap();
    let webauthn = create_webauthn();
    let repo = Repo::new(db.clone());

    if let Err(err) = ensure_default_api_token(&repo).await {
        tracing::error!("Failed to create default API token: {}", err);
    }

    let state = AppState {
        leptos_options: leptos_options.clone(),
        routes: routes.clone(),
        repo,
        webauthn,
    };
    let session_store = PostgresStore::new(db);
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("guardrail")
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::hours(4)))
        .with_secure(false);

    let auth_layer = AuthLayer::new();

    // Build our router with all routes and middleware
    let routes_all = Router::new()
        .route("/api/{*fn_name}", axum::routing::get(server_fn_handler).post(server_fn_handler))
        .leptos_routes_with_handler(routes, axum::routing::get(leptos_routes_handler))
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .nest("/api", api::routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(auth_layer)
        .layer(session_layer)
        .with_state(state);

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    //TODO: Make configurable
    let config = RustlsConfig::from_pem_file(
        PathBuf::from("dev").join("cert.pem"),
        PathBuf::from("dev").join("key.pem"),
    )
    .await
    .unwrap();

    let port = settings().server.port;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    axum_server::bind_rustls(addr, config)
        .serve(routes_all.into_make_service())
        .await
        .unwrap();
}
