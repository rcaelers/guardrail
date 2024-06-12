mod api;
mod app_state;
mod auth;
mod fileserv;
mod session_store;
mod utils;

use app::auth::layer::AuthLayer;
use app::auth::AuthSession;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use fileserv::file_and_error_handler;
use leptos::*;
use leptos_axum::{generate_route_list, handle_server_fns_with_context, LeptosRoutes};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use time::Duration;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::level_filters::LevelFilter;
use tracing::{info, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, FmtSubscriber};
use webauthn_rs::prelude::*;

use crate::entity;
use app::settings::settings;
use app::*;
use app_state::AppState;
use session_store::SeaOrmSessionStore;

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
        .add_directive("app=debug".parse().unwrap());

    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(filter)
        .finish()
        .with(fmt::Layer::new().with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

async fn init_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let connect_options = ConnectOptions::new(&settings().database.uri).to_owned();
    Database::connect(connect_options).await
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
            provide_context(app_state.db.clone());
            provide_context(auth_session.clone());
            provide_context(auth_session.user.clone());
        },
        request,
    )
    .await
}

async fn leptos_routes_handler(
    auth_session: AuthSession,
    State(app_state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let handler = leptos_axum::render_route_with_context(
        app_state.leptos_options.clone(),
        app_state.routes.clone(),
        move || {
            provide_context(app_state.db.clone());
            provide_context(auth_session.clone());
            provide_context(auth_session.user.clone());
        },
        app::App,
    );
    handler(req).await.into_response()
}

#[tokio::main]
async fn main() {
    init_logging().await;

    info!("Starting server on port {}", settings().server.port);

    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let _addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let db = init_db().await.unwrap();
    let webauthn = create_webauthn();
    let state = AppState {
        leptos_options: leptos_options.clone(),
        routes: routes.clone(),
        db: db.clone(),
        webauthn,
    };

    let session_store = SeaOrmSessionStore::new(db);
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("guardrail")
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::hours(1)))
        .with_secure(false);

    let auth_layer = AuthLayer::new();

    let routes_all = Router::new()
        .route(
            "/api/*fn_name",
            axum::routing::get(server_fn_handler).post(server_fn_handler),
        )
        .leptos_routes_with_handler(routes, axum::routing::get(leptos_routes_handler))
        .fallback(file_and_error_handler)
        .nest("/api", api::routes().await)
        .nest("/auth", auth::routes().await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(auth_layer)
        .layer(session_layer)
        .with_state(state);

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
