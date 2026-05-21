use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{Next, from_fn},
    response::Response,
    routing::get,
};
use axum_server::tls_rustls::RustlsConfig;
use common::init_s3_object_store;
use common::retry_startup;
use repos::Repo;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer, cookie::SameSite};
use tracing::{Level, info};
use url::Url;

use crate::access::SESSION_KEY;
use crate::auth_cache::AuthCache;
use crate::auth_user::AuthenticatedUser;
use crate::pocket_id;
use crate::provisioner::IdentityProvisioner;
use crate::rauthy;
use crate::routes::{auth, db_api, home, impersonation, invite};
use crate::settings::Settings;
use crate::state::AppState;
use email::EmailSender;

async fn log_request(session: Session, request: Request, next: Next) -> Response {
    let user = session
        .get::<AuthenticatedUser>(SESSION_KEY)
        .await
        .ok()
        .flatten();

    let user_id = user.as_ref().and_then(|u| u.user.as_ref()).map(|u| u.id.as_str()).unwrap_or("anonymous");
    let real_user_id = user.as_ref().and_then(|u| u.real_user.as_ref()).map(|u| u.id.as_str());

    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or("").to_owned();

    match real_user_id {
        Some(real) => tracing::info!(method = %method, path, query, user_id, impersonated_by = real, "request"),
        None => tracing::info!(method = %method, path, query, user_id, "request"),
    }

    next.run(request).await
}

pub struct GuardrailWebApp {
    state: AppState,
}

impl GuardrailWebApp {
    // Requires live SurrealDB and object-storage configuration; covered by deployment/e2e smoke tests.
    pub async fn from_settings(settings: Arc<Settings>) -> Self {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let db = retry_startup("SurrealDB", || {
            let settings = settings.clone();
            async move {
                let db = surrealdb::engine::any::connect(&settings.database.endpoint).await?;

                db.signin(surrealdb::opt::auth::Root {
                    username: settings.database.username.clone(),
                    password: settings.database.password.clone(),
                })
                .await?;

                db.use_ns(&settings.database.namespace)
                    .use_db(&settings.database.database)
                    .await?;

                Ok::<_, surrealdb::Error>(db)
            }
        })
        .await;

        // Register the JWT access method so RLS $auth variables are populated.
        // OVERWRITE makes this idempotent on restart.
        {
            let public_key = &settings.database.jwk.public_key;
            db.query(format!(
                r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                    WITH JWT ALGORITHM EDDSA KEY '{public_key}'
                    AUTHENTICATE {{
                        IF $auth.id {{
                            RETURN $auth.id;
                        }};
                        IF $token.user_id {{
                            RETURN type::record('users', $token.user_id);
                        }};
                        IF $token.username {{
                            RETURN type::record('users', $token.username);
                        }};
                    }}
                    DURATION FOR SESSION 1h"#
            ))
            .await
            .expect("Failed to define JWT access method");
        }

        let http_client = {
            let mut builder =
                reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));
            if settings
                .oidc
                .as_ref()
                .and_then(|oidc| oidc.allow_insecure_tls)
                .unwrap_or(false)
            {
                builder = builder.danger_accept_invalid_certs(true);
            }
            builder.build().expect("Failed to build HTTP client")
        };

        let provisioner: Option<Arc<dyn IdentityProvisioner>> =
            if let Some(cfg) = settings.provisioner.pocket_id.as_ref() {
                let api_url =
                    Url::parse(&cfg.api_url).expect("Invalid provisioner.pocket_id.api_url");
                let public_url = cfg
                    .public_url
                    .as_deref()
                    .map(|u| Url::parse(u).expect("Invalid provisioner.pocket_id.public_url"))
                    .unwrap_or_else(|| api_url.clone());
                let setup_path = cfg.setup_path.clone().unwrap_or_else(|| "/lc/".to_string());
                let post_setup_redirect = cfg.post_setup_redirect.clone().or_else(|| {
                    settings
                        .oidc
                        .as_ref()
                        .and_then(|o| o.launch_url.as_deref())
                        .filter(|u| !u.is_empty())
                        .map(|launch_url| {
                            format!("{}/auth/login/start", launch_url.trim_end_matches('/'))
                        })
                });
                Some(Arc::new(pocket_id::PocketIdProvisioner {
                    api_url,
                    public_url,
                    api_key: cfg.api_key.clone(),
                    setup_path,
                    post_setup_redirect,
                    client: http_client.clone(),
                }) as Arc<dyn IdentityProvisioner>)
            } else if let Some(cfg) = settings.provisioner.rauthy.as_ref() {
                let api_url =
                    Url::parse(&cfg.api_url).expect("Invalid provisioner.rauthy.api_url");
                let public_url = cfg
                    .public_url
                    .as_deref()
                    .map(|u| Url::parse(u).expect("Invalid provisioner.rauthy.public_url"))
                    .unwrap_or_else(|| api_url.clone());
                Some(Arc::new(rauthy::RauthyProvisioner {
                    api_url,
                    public_url,
                    api_key: cfg.api_key.clone(),
                    client: http_client.clone(),
                }) as Arc<dyn IdentityProvisioner>)
            } else {
                None
            };

        let storage = init_s3_object_store(&settings.object_storage).await;

        let email_sender: Option<Arc<dyn EmailSender>> = if settings.email.from.is_empty() {
            None
        } else if let Some(key) = settings
            .email
            .resend
            .as_ref()
            .map(|r| r.key.as_str())
            .filter(|k| !k.is_empty())
        {
            Some(Arc::new(email::ResendEmailSender::new(key.to_string())))
        } else {
            Some(Arc::new(email::LogEmailSender))
        };

        let state = AppState {
            repo: Arc::new(Repo::new(db)),
            settings,
            http_client,
            provisioner,
            email_sender,
            storage,
            auth_cache: AuthCache::default(),
        };

        Self { state }
    }

    // Starts a long-running HTTP/TLS listener; endpoint behavior is covered through router-level tests.
    pub async fn serve(&self) {
        let state = &self.state;
        let settings = &state.settings;

        let use_secure_cookies = settings
            .ingress
            .public_key
            .as_deref()
            .is_some_and(|pem| !pem.is_empty());

        let session_layer = SessionManagerLayer::new(MemoryStore::default())
            .with_name("guardrail")
            .with_same_site(SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(time::Duration::hours(4)))
            .with_secure(use_secure_cookies);

        let api_v1 = Router::new()
            .merge(db_api::router())
            .merge(invite::api_router());

        async fn live() -> StatusCode {
            StatusCode::OK
        }

        async fn ready(State(state): State<AppState>) -> StatusCode {
            match state.repo.db.health().await {
                Ok(()) => StatusCode::OK,
                Err(err) => {
                    tracing::error!("Health check failed: {err}");
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }

        let app = Router::new()
            .merge(home::router())
            .merge(auth::router())
            .merge(impersonation::router())
            .merge(invite::router())
            .nest("/api/v1", api_v1)
            .nest_service("/static", ServeDir::new("src/web/server/static"))
            .route("/live", get(live))
            .route("/ready", get(ready))
            .route("/healthz", get(|| async { "ok" }))
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .layer(from_fn(log_request))
            .layer(session_layer)
            .with_state(state.clone());

        info!("Starting web server on port {}", settings.ingress.port);

        let addr = SocketAddr::from(([0, 0, 0, 0], settings.ingress.port));
        if let (Some(public_key), Some(private_key)) =
            (settings.ingress.public_key.clone(), settings.ingress.private_key.clone())
            && !public_key.is_empty()
            && !private_key.is_empty()
        {
            let tls = RustlsConfig::from_pem(public_key.into_bytes(), private_key.into_bytes())
                .await
                .expect("Failed to load TLS configuration");
            axum_server::bind_rustls(addr, tls)
                .serve(app.into_make_service())
                .await
                .expect("Failed to serve web app");
            return;
        }

        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await
            .expect("Failed to serve web app");
    }
}
