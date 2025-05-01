mod session_store;
mod state;

use app::auth::AuthSession;
use app::auth::layer::AuthLayer;
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum_server::tls_rustls::RustlsConfig;
use common::init_logging;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list, handle_server_fns_with_context};
use repos::Repo;
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::net::SocketAddr;
use std::sync::Arc;
use time::Duration;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::info;
use webauthn_rs::prelude::*;

use app::*;
use common::settings::Settings;
use session_store::PostgresStore;
use state::AppState;

struct GuardrailApp {
    settings: Arc<Settings>,
    db: PgPool,
    repo: Repo,
    webauthn: Arc<Webauthn>,
}

impl GuardrailApp {
    async fn new() -> Self {
        let settings = Arc::new(Settings::new().expect("Failed to load settings"));
        init_logging().await;

        let db = Self::init_db(settings.clone()).await.unwrap();
        let webauthn = Self::create_webauthn(settings.clone());
        let repo = Repo::new(db.clone());

        Self {
            settings: settings.clone(),
            db,
            repo,
            webauthn,
        }
    }

    async fn init_db(settings: Arc<Settings>) -> Result<PgPool, sqlx::Error> {
        let database_url = &settings.database.uri;
        let mut opts: PgConnectOptions = database_url.parse()?;
        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Ok(pool)
    }

    fn create_webauthn(settings: Arc<Settings>) -> Arc<Webauthn> {
        let rp_id = &settings.auth.id.as_str();
        let rp_origin = Url::parse(settings.auth.origin.as_str()).expect("Invalid URL");
        let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
        let builder = builder.rp_name(settings.auth.name.as_str());

        Arc::new(builder.build().expect("Invalid configuration"))
    }

    async fn run(self) {
        info!("Starting server on port {}", self.settings.web_server.port);

        let conf = get_configuration(None).unwrap();
        let leptos_options = conf.leptos_options;
        let _addr = leptos_options.site_addr;
        let routes = generate_route_list(App);

        let state = AppState {
            leptos_options: leptos_options.clone(),
            routes: routes.clone(),
            repo: self.repo.clone(),
            webauthn: self.webauthn.clone(),
        };
        let session_store = PostgresStore::new(self.db.clone());
        let session_layer = SessionManagerLayer::new(session_store)
            .with_name("guardrail")
            .with_same_site(SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(Duration::hours(4)))
            .with_secure(false);

        let auth_layer = AuthLayer::new();

        let routes_all = Router::new()
            .route("/api/{*fn_name}", axum::routing::get(server_fn_handler).post(server_fn_handler))
            .leptos_routes_with_handler(routes, axum::routing::get(leptos_routes_handler))
            .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
            .layer(TraceLayer::new_for_http())
            .layer(auth_layer)
            .layer(session_layer)
            .with_state(state);

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        if self.settings.web_server.public_key.is_some()
            && self.settings.web_server.private_key.is_some()
        {
            let config = RustlsConfig::from_pem(
                self.settings
                    .api_server
                    .public_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
                self.settings
                    .api_server
                    .private_key
                    .clone()
                    .unwrap_or_default()
                    .into_bytes(),
            )
            .await
            .unwrap();

            let port = self.settings.clone().web_server.port;
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            axum_server::bind_rustls(addr, config)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        } else {
            let port = self.settings.clone().web_server.port;
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            axum_server::bind(addr)
                .serve(routes_all.into_make_service())
                .await
                .unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    let app = GuardrailApp::new().await;
    app.run().await;
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
