use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use email::Email;
use serde::Deserialize;
use serde_json::Value;
use tower_sessions::Session;

use crate::{
    AppState, access,
    error::{AppError, AppResult},
    oidc,
};

pub(crate) const DEFAULT_RECOVERY_HTML: &str = include_str!("../../templates/email/recovery.html");
pub(crate) const DEFAULT_RECOVERY_TEXT: &str = include_str!("../../templates/email/recovery.txt");
pub(crate) const DEFAULT_RECOVERY_SUBJECT: &str = "Your one-time login link";

fn render_recovery_template(template: &str, recovery_url: &str) -> String {
    template.replace("{{recovery_url}}", recovery_url)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(oidc::login_start))
        .route("/auth/login/start", get(oidc::login_start))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/auth/logout", post(logout))
        .route("/auth/real-user", get(get_real_user))
        .route("/auth/recovery", post(request_recovery))
}

#[derive(Deserialize)]
struct RecoveryRequest {
    email: String,
}

async fn request_recovery(
    State(state): State<AppState>,
    Json(body): Json<RecoveryRequest>,
) -> AppResult<Json<Value>> {
    // Look up the user by email. We proceed silently regardless of outcome to
    // avoid leaking whether a given email is registered.
    // Returns the login URL directly when no email sender is configured (dev mode).
    let result: AppResult<Option<String>> = async {
        let user = repos::user::UserRepo::get_by_email(&state.repo.db, &body.email)
            .await
            .map_err(AppError::internal)?;
        let Some(user) = user else { return Ok(None); };

        let provisioner = state.provisioner.as_ref().ok_or_else(|| {
            AppError::failure("No identity provisioner configured")
        })?;

        let pocket_id = provisioner
            .find_user_id(&user.email, &user.username)
            .await
            .map_err(|e| {
                tracing::warn!(email = %body.email, "failed to look up user in identity provider: {e}");
                AppError::failure(e.to_string())
            })?
            .ok_or_else(|| AppError::not_found("User not found in identity provider"))?;

        let recovery_url = provisioner
            .create_recovery_url(&pocket_id)
            .await
            .map_err(|e| {
                tracing::warn!(email = %body.email, "failed to create recovery URL: {e}");
                AppError::failure(e.to_string())
            })?;

        if let Some(sender) = state.email_sender.as_deref() {
            let app_settings = repos::app_settings::AppSettingsRepo::get_or_create(&state.repo.db)
                .await
                .unwrap_or_default();
            let subject = app_settings.email.recovery_subject
                .unwrap_or_else(|| DEFAULT_RECOVERY_SUBJECT.to_string());
            let html_template = app_settings.email.recovery_html_template
                .unwrap_or_else(|| DEFAULT_RECOVERY_HTML.to_string());
            let text_template = app_settings.email.recovery_text_template
                .unwrap_or_else(|| DEFAULT_RECOVERY_TEXT.to_string());
            let url_str = recovery_url.as_str();
            let email = Email {
                from: state.settings.email.from.clone(),
                to: body.email.clone(),
                subject,
                html: render_recovery_template(&html_template, url_str),
                text: Some(render_recovery_template(&text_template, url_str)),
            };
            if let Err(e) = sender.send(email).await {
                tracing::warn!(email = %body.email, "failed to send recovery email: {e}");
            }
            Ok(None)
        } else {
            tracing::info!(email = %body.email, url = %recovery_url, "recovery URL generated (no email sender configured)");
            Ok(Some(recovery_url.to_string()))
        }
    }
    .await;

    let login_url = match result {
        Ok(url) => url,
        Err(e) => {
            tracing::warn!(email = %body.email, "recovery request failed: {e}");
            None
        }
    };

    Ok(Json(serde_json::json!({ "ok": true, "login_url": login_url })))
}

async fn logout(State(state): State<AppState>, session: Session) -> impl IntoResponse {
    let id_token = session
        .get::<crate::auth_user::AuthenticatedUser>(access::SESSION_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|u| u.id_token);
    let _ = session.flush().await;
    let redirect_to = oidc::end_session_url(&state, id_token.as_deref())
        .await
        .unwrap_or_else(|| "/".to_string());
    let mut response = Redirect::to(&redirect_to).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_static("gr_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_static("gr_real_uid=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response
}

/// Returns the real (admin) user's data when impersonation is active.
/// Reads from the session (trusted server-side state) and queries root DB.
/// 404 if not currently impersonating.
async fn get_real_user(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    let user = access::require_session(&session).await?;
    let admin_id = user
        .real_user
        .ok_or_else(|| AppError::not_found("not impersonating"))?
        .id;

    let mut result = state
        .repo
        .db
        .query(
            "SELECT meta::id(id) AS id, email, name, avatar, \
             is_admin AS isAdmin, created_at AS joinedAt \
             FROM ONLY type::record('users', $id)",
        )
        .bind(("id", admin_id))
        .await
        .map_err(AppError::internal)?;

    let rows: Vec<Value> = result.take(0).map_err(AppError::internal)?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| AppError::not_found("real user not found"))
}
