use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use common::settings::Oidc;

use crate::auth_user::{AuthenticatedUser, User};
use data::user::NewUser;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_sessions::Session;
use url::{Url, form_urlencoded};
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, AppResult},
};

const OIDC_SCOPE: &str = "openid profile email";
const OIDC_LOGIN_SESSION_KEY: &str = "oidc_login_state";
const AUTHENTICATED_USER_SESSION_KEY: &str = "authenticated_user";

#[derive(Debug, Deserialize)]
pub struct LoginStartQuery {
    pub next: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcDiscoveryDocument {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    end_session_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct OidcUserInfo {
    sub: String,
    preferred_username: Option<String>,
    email: Option<String>,
    name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OidcLoginState {
    csrf_state: String,
    next: String,
    code_verifier: Option<String>,
}

// Performs outbound OIDC discovery and redirects through an external identity provider.
pub async fn login_start(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<LoginStartQuery>,
) -> AppResult<Redirect> {
    if session
        .get::<AuthenticatedUser>(AUTHENTICATED_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .is_some_and(|a| a.is_authenticated())
    {
        return Ok(Redirect::to(sanitize_next(query.next.as_deref()).as_str()));
    }

    let oidc = oidc_settings(&state)?;
    let discovery = fetch_discovery(&state, oidc).await?;
    let next = sanitize_next(query.next.as_deref());
    let csrf_state = Uuid::new_v4().to_string();
    let pkce = oidc.pkce.unwrap_or(true);
    let (code_verifier, code_challenge) = if pkce {
        let verifier = generate_code_verifier();
        let challenge = derive_code_challenge(&verifier);
        (Some(verifier), Some(challenge))
    } else {
        (None, None)
    };
    let session_state = OidcLoginState {
        csrf_state: csrf_state.clone(),
        next,
        code_verifier,
    };

    session
        .insert(OIDC_LOGIN_SESSION_KEY, session_state)
        .await
        .map_err(AppError::internal)?;

    let prompt = sanitize_prompt(query.prompt.as_deref());
    let authorize_url = build_authorize_url(
        &discovery.authorization_endpoint,
        oidc,
        &csrf_state,
        code_challenge.as_deref(),
        prompt.as_deref(),
    )?;
    Ok(Redirect::to(authorize_url.as_str()))
}

// Performs the full OIDC callback exchange against external provider endpoints.
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<OidcCallbackQuery>,
) -> AppResult<Response> {
    let oidc = oidc_settings(&state)?;
    let login_state = session
        .remove::<OidcLoginState>(OIDC_LOGIN_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::corrupt_session)?;

    if let Some(error) = query.error {
        let description = query
            .error_description
            .unwrap_or_else(|| "OIDC login failed".to_string());
        let message = format!("{error}: {description}");
        return Ok(Redirect::to(
            login_path(Some(login_state.next.as_str()), Some(message.as_str())).as_str(),
        )
        .into_response());
    }

    let state_value = query
        .state
        .as_deref()
        .ok_or_else(|| AppError::failure("missing OIDC state"))?;
    if state_value != login_state.csrf_state {
        return Ok(Redirect::to(
            login_path(Some(login_state.next.as_str()), Some("invalid OIDC state")).as_str(),
        )
        .into_response());
    }

    let code = query
        .code
        .as_deref()
        .ok_or_else(|| AppError::failure("missing authorization code"))?;

    let discovery = fetch_discovery(&state, oidc).await?;
    let token = exchange_code(
        &state,
        &discovery.token_endpoint,
        oidc,
        code,
        login_state.code_verifier.as_deref(),
    )
    .await?;
    let userinfo =
        fetch_userinfo(&state, &discovery.userinfo_endpoint, &token.access_token).await?;
    let username = resolve_username(&userinfo);

    let authenticated_user =
        get_or_create_local_user(&state, &userinfo.sub, &username, userinfo.email.as_deref())
            .await
            .map_err(|e| {
                AppError::internal(format!("failed to get or create user '{username}': {e}"))
            })?;

    let Some(authenticated_user) = authenticated_user else {
        return Ok(Redirect::to(
            login_path(
                Some(login_state.next.as_str()),
                Some("Your account has not been granted access to Guardrail. Contact an administrator."),
            )
            .as_str(),
        )
        .into_response());
    };

    session
        .insert(AUTHENTICATED_USER_SESSION_KEY, authenticated_user.clone())
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to(login_state.next.as_str()).into_response())
}

fn oidc_settings(state: &AppState) -> AppResult<&Oidc> {
    let oidc = state.settings.oidc.as_ref().ok_or_else(|| {
        AppError::failure(
            "OIDC settings are missing. Set the GUARDRAIL_AUTH_OIDC_* environment variables first.",
        )
    })?;
    if oidc.issuer_url.is_empty()
        || oidc.client_id.is_empty()
        || oidc.client_secret.is_empty()
        || oidc.callback_url.is_empty()
    {
        return Err(AppError::failure(
            "OIDC settings are missing. Set the GUARDRAIL_AUTH_OIDC_* environment variables first.",
        ));
    }

    Ok(oidc)
}

async fn fetch_discovery(state: &AppState, oidc: &Oidc) -> AppResult<OidcDiscoveryDocument> {
    let issuer = oidc
        .internal_issuer_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(oidc.issuer_url.as_str())
        .trim_end_matches('/');
    let discovery_url = format!("{issuer}/.well-known/openid-configuration");
    let response = state
        .http_client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| {
            AppError::internal(format!("OIDC discovery request to {discovery_url} failed: {e}"))
        })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "OIDC discovery at {discovery_url} returned {status}: {body}"
        )));
    }
    let mut discovery = response
        .json::<OidcDiscoveryDocument>()
        .await
        .map_err(|e| AppError::internal(format!("OIDC discovery response parse error: {e}")))?;
    if oidc
        .internal_issuer_url
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        rewrite_internal_endpoint(&mut discovery.token_endpoint, oidc);
        rewrite_internal_endpoint(&mut discovery.userinfo_endpoint, oidc);
    }
    Ok(discovery)
}

fn rewrite_internal_endpoint(endpoint: &mut String, oidc: &Oidc) {
    let Some(internal_issuer) = oidc
        .internal_issuer_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/'))
    else {
        return;
    };
    let public_issuer = oidc.issuer_url.trim_end_matches('/');
    if let Some(path) = endpoint.strip_prefix(public_issuer) {
        *endpoint = format!("{internal_issuer}{path}");
    }
}

/// Returns the IdP end_session URL with `post_logout_redirect_uri` set, or `None`
/// if the discovery document doesn't advertise one or settings are unavailable.
pub async fn end_session_url(state: &AppState) -> Option<String> {
    let oidc = oidc_settings(state).ok()?;
    let post_logout_redirect = oidc.logout_callback_url.as_str();
    if post_logout_redirect.is_empty() {
        return None;
    }
    let discovery = fetch_discovery(state, oidc).await.ok()?;
    let endpoint = discovery.end_session_endpoint?;
    let mut url = Url::parse(&endpoint).ok()?;
    url.query_pairs_mut()
        .append_pair("post_logout_redirect_uri", post_logout_redirect);
    Some(url.into())
}

fn sanitize_prompt(prompt: Option<&str>) -> Option<&str> {
    match prompt {
        Some("login") | Some("none") | Some("consent") | Some("select_account") => prompt,
        _ => None,
    }
}

fn build_authorize_url(
    authorization_endpoint: &str,
    oidc: &Oidc,
    csrf_state: &str,
    code_challenge: Option<&str>,
    prompt: Option<&str>,
) -> AppResult<Url> {
    let mut url = Url::parse(authorization_endpoint).map_err(AppError::internal)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("response_type", "code")
            .append_pair("client_id", oidc.client_id.as_str())
            .append_pair("redirect_uri", oidc.callback_url.as_str())
            .append_pair("scope", OIDC_SCOPE)
            .append_pair("state", csrf_state);
        if let Some(p) = prompt {
            pairs.append_pair("prompt", p);
        }
        if let Some(challenge) = code_challenge {
            pairs
                .append_pair("code_challenge", challenge)
                .append_pair("code_challenge_method", "S256");
        }
    }
    Ok(url)
}

async fn exchange_code(
    state: &AppState,
    token_endpoint: &str,
    oidc: &Oidc,
    code: &str,
    code_verifier: Option<&str>,
) -> AppResult<OidcTokenResponse> {
    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", oidc.callback_url.as_str()),
        ("client_id", oidc.client_id.as_str()),
        ("client_secret", oidc.client_secret.as_str()),
    ];
    if let Some(verifier) = code_verifier {
        params.push(("code_verifier", verifier));
    }
    let response = state
        .http_client
        .post(token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            AppError::internal(format!(
                "OIDC token exchange request to {token_endpoint} failed: {e}"
            ))
        })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "OIDC token exchange at {token_endpoint} returned {status}: {body}"
        )));
    }
    response
        .json::<OidcTokenResponse>()
        .await
        .map_err(|e| AppError::internal(format!("OIDC token response parse error: {e}")))
}

async fn fetch_userinfo(
    state: &AppState,
    userinfo_endpoint: &str,
    access_token: &str,
) -> AppResult<OidcUserInfo> {
    let response = state
        .http_client
        .get(userinfo_endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            AppError::internal(format!("OIDC userinfo request to {userinfo_endpoint} failed: {e}"))
        })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "OIDC userinfo at {userinfo_endpoint} returned {status}: {body}"
        )));
    }
    response
        .json::<OidcUserInfo>()
        .await
        .map_err(|e| AppError::internal(format!("OIDC userinfo response parse error: {e}")))
}

fn generate_code_verifier() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn derive_code_challenge(code_verifier: &str) -> String {
    let hash = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn resolve_username(userinfo: &OidcUserInfo) -> String {
    userinfo
        .preferred_username
        .clone()
        .or_else(|| userinfo.email.clone())
        .or_else(|| userinfo.name.clone())
        .unwrap_or_else(|| userinfo.sub.clone())
}

async fn get_or_create_local_user(
    state: &AppState,
    sub: &str,
    username: &str,
    email: Option<&str>,
) -> AppResult<Option<AuthenticatedUser>> {
    let pending = repos::pending_access::PendingAccessRepo::get_by_sub(&state.repo.db, sub)
        .await
        .map_err(AppError::internal)?;

    // Look up by username first, then fall back to email if the OIDC provider
    // returns an email as the username but the stored record uses a different username.
    let existing = repos::user::UserRepo::get_by_name(&state.repo.db, username)
        .await
        .map_err(AppError::internal)?
        .or(match email {
            Some(e) => repos::user::UserRepo::get_by_email(&state.repo.db, e)
                .await
                .map_err(AppError::internal)?,
            None => None,
        });

    if let Some(user) = existing {
        if let Some(pa) = pending {
            repos::pending_access::PendingAccessRepo::apply_and_delete(
                &state.repo.db,
                &user.id,
                &pa,
            )
            .await
            .map_err(AppError::internal)?;
        }
        return Ok(Some(AuthenticatedUser::authenticated(User {
            id: user.id,
            name: user.username,
            is_admin: user.is_admin,
            avatar: None,
        })));
    }

    // No existing user — only create one if they arrived via an invitation.
    let Some(pa) = pending else {
        return Ok(None);
    };

    let user_id = repos::user::UserRepo::create(
        &state.repo.db,
        NewUser {
            username: username.to_owned(),
            email: email.map(str::to_string),
            is_admin: pa.is_admin,
        },
    )
    .await
    .map_err(AppError::internal)?;

    repos::pending_access::PendingAccessRepo::apply_and_delete(&state.repo.db, &user_id, &pa)
        .await
        .map_err(AppError::internal)?;

    Ok(Some(AuthenticatedUser::authenticated(User {
        id: user_id,
        name: username.to_owned(),
        is_admin: pa.is_admin,
        avatar: None,
    })))
}

pub fn sanitize_next(next: Option<&str>) -> String {
    let next = next.unwrap_or("/");
    if next.starts_with('/') && !next.starts_with("//") {
        return next.to_string();
    }
    "/".to_string()
}

pub fn login_path(next: Option<&str>, error: Option<&str>) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let next = sanitize_next(next);
    serializer.append_pair("next", next.as_str());
    if let Some(error) = error.filter(|value| !value.is_empty()) {
        serializer.append_pair("error", error);
    }
    format!("/login?{}", serializer.finish())
}

pub fn login_start_path(next: Option<&str>) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let next = sanitize_next(next);
    serializer.append_pair("next", next.as_str());
    format!("/auth/login/start?{}", serializer.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_cache::AuthCache;
    use object_store::memory::InMemory;
    use repos::Repo;
    use std::sync::Arc;

    fn oidc() -> Oidc {
        Oidc {
            issuer_url: "https://issuer.example".to_string(),
            internal_issuer_url: None,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            callback_url: "https://guardrail.example/auth/oidc/callback".to_string(),
            logout_callback_url: String::new(),
            launch_url: None,
            self_service_url: None,
            pkce: Some(true),
            allow_insecure_tls: None,
        }
    }

    async fn state_with_oidc(oidc: Option<Oidc>) -> AppState {
        testware::setup::TestSetup::init();
        let db = testware::setup::TestSetup::create_db().await;
        let mut settings = crate::settings::Settings::test_default();
        settings.oidc = oidc;
        let storage: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
        AppState {
            repo: Arc::new(Repo::new(db)),
            settings: Arc::new(settings),
            http_client: reqwest::Client::new(),
            provisioner: None,
            email_sender: None,
            storage,
            auth_cache: AuthCache::default(),
        }
    }

    #[tokio::test]
    async fn oidc_settings_validate_presence_and_required_fields() {
        let missing = state_with_oidc(None).await;
        assert!(matches!(oidc_settings(&missing), Err(AppError::Failure(_))));

        let mut incomplete = oidc();
        incomplete.client_secret.clear();
        let incomplete = state_with_oidc(Some(incomplete)).await;
        assert!(matches!(oidc_settings(&incomplete), Err(AppError::Failure(_))));

        let valid = state_with_oidc(Some(oidc())).await;
        assert_eq!(oidc_settings(&valid).unwrap().client_id, "client");
    }

    #[test]
    fn internal_endpoint_rewrite_only_applies_to_public_issuer_prefix() {
        let mut oidc = oidc();
        let mut endpoint = "https://issuer.example/token".to_string();
        rewrite_internal_endpoint(&mut endpoint, &oidc);
        assert_eq!(endpoint, "https://issuer.example/token");

        oidc.internal_issuer_url = Some("http://issuer-internal".to_string());
        rewrite_internal_endpoint(&mut endpoint, &oidc);
        assert_eq!(endpoint, "http://issuer-internal/token");

        let mut other = "https://other.example/token".to_string();
        rewrite_internal_endpoint(&mut other, &oidc);
        assert_eq!(other, "https://other.example/token");
    }

    #[test]
    fn build_authorize_url_includes_required_oidc_and_pkce_params() {
        let oidc = oidc();
        let url = build_authorize_url(
            "https://issuer.example/authorize",
            &oidc,
            "csrf",
            Some("challenge"),
            Some("login"),
        )
        .unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

        assert_eq!(params.get("response_type").unwrap(), "code");
        assert_eq!(params.get("client_id").unwrap(), "client");
        assert_eq!(
            params.get("redirect_uri").unwrap(),
            "https://guardrail.example/auth/oidc/callback"
        );
        assert_eq!(params.get("scope").unwrap(), OIDC_SCOPE);
        assert_eq!(params.get("state").unwrap(), "csrf");
        assert_eq!(params.get("prompt").unwrap(), "login");
        assert_eq!(params.get("code_challenge").unwrap(), "challenge");
        assert_eq!(params.get("code_challenge_method").unwrap(), "S256");

        let no_pkce =
            build_authorize_url("https://issuer.example/authorize", &oidc, "csrf", None, None)
                .unwrap();
        assert!(!no_pkce.query().unwrap_or("").contains("code_challenge"));
        assert!(!no_pkce.query().unwrap_or("").contains("prompt"));
    }

    #[test]
    fn sanitize_prompt_allows_known_values_and_rejects_unknown() {
        assert_eq!(sanitize_prompt(None), None);
        assert_eq!(sanitize_prompt(Some("login")), Some("login"));
        assert_eq!(sanitize_prompt(Some("none")), Some("none"));
        assert_eq!(sanitize_prompt(Some("consent")), Some("consent"));
        assert_eq!(sanitize_prompt(Some("select_account")), Some("select_account"));
        assert_eq!(sanitize_prompt(Some("evil")), None);
        assert_eq!(sanitize_prompt(Some("")), None);
    }

    #[test]
    fn pkce_and_username_helpers_cover_fallbacks() {
        let verifier = generate_code_verifier();
        assert!(!verifier.is_empty());
        assert_eq!(derive_code_challenge("abc"), "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0");

        assert_eq!(
            resolve_username(&OidcUserInfo {
                sub: "sub".into(),
                preferred_username: Some("preferred".into()),
                email: Some("email@example.com".into()),
                name: Some("Name".into()),
            }),
            "preferred"
        );
        assert_eq!(
            resolve_username(&OidcUserInfo {
                sub: "sub".into(),
                preferred_username: None,
                email: Some("email@example.com".into()),
                name: Some("Name".into()),
            }),
            "email@example.com"
        );
        assert_eq!(
            resolve_username(&OidcUserInfo {
                sub: "sub".into(),
                preferred_username: None,
                email: None,
                name: Some("Name".into()),
            }),
            "Name"
        );
        assert_eq!(
            resolve_username(&OidcUserInfo {
                sub: "sub".into(),
                preferred_username: None,
                email: None,
                name: None,
            }),
            "sub"
        );
    }

    #[test]
    fn login_paths_sanitize_redirect_targets() {
        assert_eq!(sanitize_next(Some("/dashboard")), "/dashboard");
        assert_eq!(sanitize_next(Some("//evil.example")), "/");
        assert_eq!(sanitize_next(Some("https://evil.example")), "/");
        assert_eq!(sanitize_next(None), "/");

        assert_eq!(login_path(Some("/dashboard"), None), "/login?next=%2Fdashboard");
        assert_eq!(
            login_path(Some("/dashboard"), Some("bad login")),
            "/login?next=%2Fdashboard&error=bad+login"
        );
        assert_eq!(login_start_path(Some("/dashboard")), "/auth/login/start?next=%2Fdashboard");
    }
}
