use axum::{
    extract::{Query, State},
    response::Redirect,
};
use common::{AuthenticatedUser, settings::Oidc};
use data::user::NewUser;
use serde::{Deserialize, Serialize};
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
}

pub async fn login_start(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<LoginStartQuery>,
) -> AppResult<Redirect> {
    if session
        .get::<AuthenticatedUser>(AUTHENTICATED_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .is_some()
    {
        return Ok(Redirect::to(sanitize_next(query.next.as_deref()).as_str()));
    }

    let oidc = oidc_settings(&state)?;
    let discovery = fetch_discovery(&state, oidc).await?;
    let next = sanitize_next(query.next.as_deref());
    let csrf_state = Uuid::new_v4().to_string();
    let session_state = OidcLoginState {
        csrf_state: csrf_state.clone(),
        next,
    };

    session
        .insert(OIDC_LOGIN_SESSION_KEY, session_state)
        .await
        .map_err(AppError::internal)?;

    let authorize_url = build_authorize_url(&discovery.authorization_endpoint, oidc, &csrf_state)?;
    Ok(Redirect::to(authorize_url.as_str()))
}

pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<OidcCallbackQuery>,
) -> AppResult<Redirect> {
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
            home_path(Some(login_state.next.as_str()), Some(message.as_str())).as_str(),
        ));
    }

    let state_value = query
        .state
        .as_deref()
        .ok_or_else(|| AppError::failure("missing OIDC state"))?;
    if state_value != login_state.csrf_state {
        return Ok(Redirect::to(
            home_path(Some(login_state.next.as_str()), Some("invalid OIDC state")).as_str(),
        ));
    }

    let code = query
        .code
        .as_deref()
        .ok_or_else(|| AppError::failure("missing authorization code"))?;

    let discovery = fetch_discovery(&state, oidc).await?;
    let token = exchange_code(&state, &discovery.token_endpoint, oidc, code).await?;
    let userinfo =
        fetch_userinfo(&state, &discovery.userinfo_endpoint, &token.access_token).await?;
    let username = resolve_username(&userinfo);

    let authenticated_user = get_or_create_local_user(&state, &username).await?;
    session
        .insert(AUTHENTICATED_USER_SESSION_KEY, authenticated_user)
        .await
        .map_err(AppError::internal)?;

    Ok(Redirect::to(login_state.next.as_str()))
}

fn oidc_settings(state: &AppState) -> AppResult<&Oidc> {
    let oidc = &state.settings.auth.oidc;
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
    let issuer = oidc.issuer_url.trim_end_matches('/');
    let discovery_url = format!("{issuer}/.well-known/openid-configuration");
    state
        .http_client
        .get(discovery_url)
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json::<OidcDiscoveryDocument>()
        .await
        .map_err(AppError::internal)
}

fn build_authorize_url(
    authorization_endpoint: &str,
    oidc: &Oidc,
    csrf_state: &str,
) -> AppResult<Url> {
    let mut url = Url::parse(authorization_endpoint).map_err(AppError::internal)?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", oidc.client_id.as_str())
        .append_pair("redirect_uri", oidc.callback_url.as_str())
        .append_pair("scope", OIDC_SCOPE)
        .append_pair("state", csrf_state);
    Ok(url)
}

async fn exchange_code(
    state: &AppState,
    token_endpoint: &str,
    oidc: &Oidc,
    code: &str,
) -> AppResult<OidcTokenResponse> {
    state
        .http_client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", oidc.callback_url.as_str()),
            ("client_id", oidc.client_id.as_str()),
            ("client_secret", oidc.client_secret.as_str()),
        ])
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json::<OidcTokenResponse>()
        .await
        .map_err(AppError::internal)
}

async fn fetch_userinfo(
    state: &AppState,
    userinfo_endpoint: &str,
    access_token: &str,
) -> AppResult<OidcUserInfo> {
    state
        .http_client
        .get(userinfo_endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json::<OidcUserInfo>()
        .await
        .map_err(AppError::internal)
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
    username: &str,
) -> AppResult<AuthenticatedUser> {
    if let Some(user) = repos::user::UserRepo::get_by_name(&state.repo.db, username)
        .await
        .map_err(AppError::internal)?
    {
        return Ok(AuthenticatedUser::new(user.id, user.username, user.is_admin));
    }

    let is_first_user = repos::user::UserRepo::count(&state.repo.db)
        .await
        .map_err(AppError::internal)?
        == 0;
    let user_id = repos::user::UserRepo::create(
        &state.repo.db,
        NewUser {
            username: username.to_owned(),
            is_admin: is_first_user,
        },
    )
    .await
    .map_err(AppError::internal)?;

    Ok(AuthenticatedUser::new(user_id, username.to_owned(), is_first_user))
}

pub fn sanitize_next(next: Option<&str>) -> String {
    let next = next.unwrap_or("/");
    if next.starts_with('/') && !next.starts_with("//") {
        return next.to_string();
    }
    "/".to_string()
}

pub fn home_path(next: Option<&str>, error: Option<&str>) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let next = sanitize_next(next);
    serializer.append_pair("next", next.as_str());
    if let Some(error) = error.filter(|value| !value.is_empty()) {
        serializer.append_pair("error", error);
    }

    format!("/?{}", serializer.finish())
}

pub fn login_start_path(next: Option<&str>) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let next = sanitize_next(next);
    serializer.append_pair("next", next.as_str());
    format!("/auth/login/start?{}", serializer.finish())
}
