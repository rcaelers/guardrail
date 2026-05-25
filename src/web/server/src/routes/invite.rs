use axum::{
    Form, Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use data::api_token::{ApiToken, ENTITLEMENT_INVITATION_CREATE};
use data::invitation::{
    Invitation, InvitationGrant, InvitationStatus, NewInvitation, UpdateInvitation,
};
use data::pending_access::{NewPendingAccess, PendingAccessGrant};
use email::Email;
use serde::Deserialize;
use tower_sessions::Session;

use super::render;
use crate::{
    AppState, access,
    access::Principal,
    auth_user::AuthenticatedUser,
    error::{AppError, AppResult},
    provisioner::CreateUserRequest,
    templates::{InviteEmailHtml, InviteEmailText, InviteTemplate, ProductGrant},
};

pub(crate) const DEFAULT_INVITE_HTML: &str = include_str!("../../templates/email/invite.html");
pub(crate) const DEFAULT_INVITE_TEXT: &str = include_str!("../../templates/email/invite.txt");
pub(crate) const DEFAULT_INVITE_SUBJECT: &str = "You've been invited to Guardrail";

const AUTO_LOGIN_HTML: &str = include_str!("../../templates/invite-auto-login.html");

/// Renders a per-product custom email template (loaded from the DB at runtime).
/// Supports only flat substitution: {{invite_url}}, {{product_name}}, {{product_role}}.
fn render_custom_template(template: &str, invite_url: &str, products: &[ProductGrant]) -> String {
    let (name, role) = products
        .first()
        .map(|p| (p.name.as_str(), p.role.as_str()))
        .unwrap_or(("", ""));
    template
        .replace("{{invite_url}}", invite_url)
        .replace("{{product_name}}", name)
        .replace("{{product_role}}", role)
}

fn role_label(role: &str) -> &'static str {
    match role {
        "readonly" => "Read-only",
        "readwrite" => "Read & write",
        "maintainer" => "Maintainer",
        _ => "Member",
    }
}

fn invitation_has_reached_use_limit(invitation: &Invitation) -> bool {
    invitation.status == InvitationStatus::Exhausted
        || invitation
            .max_uses
            .is_some_and(|max| invitation.use_count >= max)
}

/// Invitation API routes, to be nested under /api/v1 in main.rs.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/invitations", get(list_invitations).post(create_invitation))
        .route("/invitations/{id}", put(update_invitation).delete(delete_invitation))
        .route("/invitations/{id}/revoke", axum::routing::post(revoke_invitation))
        .route("/invitations/{id}/send", axum::routing::post(send_invitation_email))
        .route("/invitations/redeem/{code}", get(get_invite_info).post(redeem_invite_json))
        .route("/invitations/redeem/{code}/setup-url", axum::routing::post(refresh_setup_url))
}

/// Web routes for the invitation redemption flow.
pub fn router() -> Router<AppState> {
    Router::new()
        // Static segment wins over /{code} — this page is served at the PocketID domain
        // via reverse proxy so its fetch("/one-time-access-token/…") is same-origin.
        .route("/invite/auto-login", get(auto_login_page))
        .route("/invite/{code}", get(show_invite_form).post(redeem_invite))
}

async fn auto_login_page() -> impl IntoResponse {
    Html(AUTO_LOGIN_HTML)
}

// --- Invitation API ---

#[derive(Deserialize)]
struct CreateInvitationRequest {
    /// If provided, an invitation email is sent to this address.
    to: Option<String>,
    is_admin: bool,
    grants: Vec<InvitationGrant>,
    expires_at: Option<DateTime<Utc>>,
    max_uses: Option<u32>,
}

#[derive(Deserialize)]
struct UpdateInvitationRequest {
    is_admin: bool,
    grants: Vec<InvitationGrant>,
    expires_at: Option<DateTime<Utc>>,
    max_uses: Option<u32>,
}

async fn list_invitations(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Json<Vec<Invitation>>> {
    let user = access::require_session(&session).await?;
    let maintained_ids =
        access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;
    let invitations = repos::invitation::InvitationRepo::get_for_user(
        &state.repo.db,
        &user.active().id,
        user.is_admin(),
        &maintained_ids,
    )
    .await
    .map_err(AppError::internal)?;
    Ok(Json(invitations))
}

async fn create_invitation(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateInvitationRequest>,
) -> AppResult<Json<Invitation>> {
    let principal = access::require_session_or_entitlement(
        &session,
        &headers,
        &state.repo.db,
        ENTITLEMENT_INVITATION_CREATE,
    )
    .await?;

    let created_by = match &principal {
        Principal::Token(token) => {
            authorize_api_token_grants(token, &body)?;
            format!("api-token:{}", token.id)
        }
        Principal::User(user) => {
            if !user.is_admin() {
                let maintained_ids =
                    access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;
                if maintained_ids.is_empty() {
                    return Err(AppError::forbidden());
                }
                if body.is_admin {
                    return Err(AppError::forbidden());
                }
                for grant in &body.grants {
                    if !maintained_ids.contains(&grant.product_id) {
                        return Err(AppError::forbidden());
                    }
                }
            }
            user.active().id.clone()
        }
    };

    let invitation = repos::invitation::InvitationRepo::create(
        &state.repo.db,
        NewInvitation {
            created_by,
            email_to: body.to.clone(),
            expires_at: body.expires_at,
            max_uses: body.max_uses,
            is_admin: body.is_admin,
            grants: body.grants,
        },
    )
    .await
    .map_err(AppError::internal)?;

    if let Some(to) = body.to.as_deref() {
        dispatch_invite_email(&state, &invitation, to).await;
    }

    Ok(Json(invitation))
}

async fn dispatch_invite_email(state: &AppState, invitation: &Invitation, to: &str) {
    let Some(sender) = state.email_sender.as_deref() else {
        tracing::warn!("invitation email not sent: no email sender configured");
        return;
    };
    let origin = state.settings.ingress.base_url.trim_end_matches('/');
    let invite_url = format!("{origin}/invite/{}", invitation.code);

    let (product_subject, product_html_template, product_text_template) = if invitation.grants.len() == 1 {
        product_email_templates(&state.repo.db, &invitation.grants[0].product_id).await
    } else {
        (None, None, None)
    };

    let products = build_products(&state.repo.db, &invitation.grants).await;

    let subject = product_subject
        .map(|t| render_custom_template(&t, &invite_url, &products))
        .unwrap_or_else(|| DEFAULT_INVITE_SUBJECT.to_string());

    use askama::Template as _;
    let html = match product_html_template {
        Some(t) => render_custom_template(&t, &invite_url, &products),
        None => InviteEmailHtml { invite_url: &invite_url, products: &products }
            .render()
            .unwrap_or_default(),
    };
    let text = match product_text_template {
        Some(t) => render_custom_template(&t, &invite_url, &products),
        None => InviteEmailText { invite_url: &invite_url, products: &products }
            .render()
            .unwrap_or_default(),
    };

    let email = Email {
        from: state.settings.email.from.clone(),
        to: to.to_string(),
        subject,
        html,
        text: Some(text),
    };
    if let Err(e) = sender.send(email).await {
        tracing::warn!(to, "failed to send invitation email: {e}");
    }
}

#[derive(Deserialize)]
struct SendInvitationEmailRequest {
    to: String,
}

async fn send_invitation_email(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<SendInvitationEmailRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user = access::require_session(&session).await?;

    let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found"))?;

    if !user.is_admin() {
        let maintained_ids =
            access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;
        let has_overlap = invitation
            .grants
            .iter()
            .any(|g| maintained_ids.contains(&g.product_id))
            || invitation.created_by == user.active().id;
        if !has_overlap {
            return Err(AppError::forbidden());
        }
    }

    if invitation_has_reached_use_limit(&invitation) {
        return Err(AppError::failure("Invitation has already been used"));
    }

    dispatch_invite_email(&state, &invitation, &body.to).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn update_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<UpdateInvitationRequest>,
) -> AppResult<Json<Invitation>> {
    let user = access::require_session(&session).await?;

    let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found"))?;

    if invitation_has_reached_use_limit(&invitation) {
        return Err(AppError::failure("Invitation has already been used"));
    }

    let (grants, is_admin) = if user.is_admin() {
        (body.grants, body.is_admin)
    } else {
        let maintained_ids =
            access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;

        let has_overlap = invitation
            .grants
            .iter()
            .any(|g| maintained_ids.contains(&g.product_id))
            || invitation.created_by == user.active().id;
        if !has_overlap {
            return Err(AppError::forbidden());
        }

        for grant in &body.grants {
            if !maintained_ids.contains(&grant.product_id) {
                return Err(AppError::forbidden());
            }
        }

        let mut merged: Vec<InvitationGrant> = invitation
            .grants
            .iter()
            .filter(|g| !maintained_ids.contains(&g.product_id))
            .cloned()
            .collect();
        merged.extend(body.grants);
        (merged, invitation.is_admin)
    };

    let updated = repos::invitation::InvitationRepo::update(
        &state.repo.db,
        &id,
        UpdateInvitation {
            expires_at: body.expires_at,
            max_uses: body.max_uses,
            is_admin,
            grants,
        },
    )
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::not_found("Invitation not found"))?;

    Ok(Json(updated))
}

async fn revoke_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = access::require_session(&session).await?;

    let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?;

    if let Some(invitation) = invitation.as_ref() {
        if invitation_has_reached_use_limit(invitation) {
            return Err(AppError::failure("Invitation has already been used"));
        }

        if !user.is_admin() {
            let maintained_ids =
                access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;
            let can_revoke = invitation.created_by == user.active().id
                || invitation
                    .grants
                    .iter()
                    .any(|g| maintained_ids.contains(&g.product_id));
            if !can_revoke {
                return Err(AppError::forbidden());
            }
        }
    } else if !user.is_admin() {
        return Err(AppError::not_found("Invitation not found"));
    }

    repos::invitation::InvitationRepo::revoke(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(serde_json::json!({ "status": "revoked" })))
}

async fn delete_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = access::require_session(&session).await?;

    if !user.is_admin() {
        let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::not_found("Invitation not found"))?;

        let maintained_ids =
            access::get_maintained_product_ids(&state.repo.db, &user.active().id).await?;
        let can_delete = invitation.created_by == user.active().id
            || invitation
                .grants
                .iter()
                .any(|g| maintained_ids.contains(&g.product_id));
        if !can_delete {
            return Err(AppError::forbidden());
        }
    }

    repos::invitation::InvitationRepo::delete(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// --- Public JSON endpoints for SvelteKit invite flow ---

async fn get_invite_info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    if repos::pending_access::PendingAccessRepo::get_by_invitation_id(
        &state.repo.db,
        &invitation.id,
    )
    .await
    .map_err(AppError::internal)?
    .is_some()
    {
        // Pending user exists — the frontend will prompt the user to open the popup,
        // which calls the refresh endpoint to get a fresh one-time token on demand.
        // We do NOT create a token here to avoid invalidating an in-flight token.
        return Ok(Json(serde_json::json!({ "valid": true, "needs_refresh": true })));
    }

    Ok(Json(serde_json::json!({ "valid": true })))
}

#[derive(Deserialize)]
struct RedeemJsonRequest {
    username: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
}

fn invitation_display_name(
    username: &str,
    first_name: Option<&str>,
    last_name: Option<&str>,
) -> String {
    let first_name = first_name.map(str::trim).filter(|value| !value.is_empty());
    let last_name = last_name.map(str::trim).filter(|value| !value.is_empty());
    let full_name = [first_name, last_name]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if full_name.is_empty() {
        username.trim().to_string()
    } else {
        full_name
    }
}

async fn redeem_invite_json(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(body): Json<RedeemJsonRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    if invitation
        .max_uses
        .is_some_and(|max| invitation.use_count >= max)
    {
        return Err(AppError::not_found("Invitation has been fully used"));
    }

    let provisioner = state
        .provisioner
        .as_ref()
        .ok_or_else(|| AppError::failure("No identity provisioner configured"))?;

    // If a PendingAccess already exists (user previously submitted the form),
    // return the stored setup URL rather than creating a new token.
    if let Some(pending) = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
        &state.repo.db,
        &invitation.id,
    )
    .await
    .map_err(AppError::internal)?
    {
        if let Some(stored_url) = pending.setup_url {
            return Ok(Json(serde_json::json!({ "setup_url": stored_url })));
        }
        // Legacy record without a stored URL — issue a fresh token.
        let setup_url = provisioner
            .create_setup_url(&pending.sub)
            .await
            .map_err(|e| {
                tracing::warn!("re-issue setup URL for invite {code}: {e}");
                AppError::failure(e.to_string())
            })?;
        return Ok(Json(match setup_url {
            Some(url) => serde_json::json!({ "setup_url": url.to_string() }),
            None => serde_json::json!({ "redirect_url": "/auth/login/start" }),
        }));
    }

    let provisioned = provisioner
        .create_user(CreateUserRequest {
            username: body.username.clone(),
            email: body.email.clone(),
            first_name: body.first_name.clone(),
            last_name: body.last_name.clone(),
        })
        .await
        .map_err(|e| {
            tracing::warn!("provisioner error for invite {code}: {e}");
            AppError::failure(e.to_string())
        })?;

    let setup_url_str = provisioned.setup_url.as_ref().map(|u| u.to_string());
    let display_name = invitation_display_name(
        &body.username,
        body.first_name.as_deref(),
        body.last_name.as_deref(),
    );

    repos::pending_access::PendingAccessRepo::create(
        &state.repo.db,
        NewPendingAccess {
            sub: provisioned.external_id.clone(),
            invitation_id: invitation.id.clone(),
            is_admin: invitation.is_admin,
            grants: invitation
                .grants
                .iter()
                .map(|g| PendingAccessGrant {
                    product_id: g.product_id.clone(),
                    role: g.role.clone(),
                })
                .collect(),
            display_name: Some(display_name),
            setup_url: setup_url_str.clone(),
        },
    )
    .await
    .map_err(AppError::internal)?;

    repos::invitation::InvitationRepo::record_acceptance(
        &state.repo.db,
        &invitation.id,
        &body.username,
        &body.email,
    )
    .await
    .map_err(AppError::internal)?;

    Ok(Json(match setup_url_str {
        Some(url) => serde_json::json!({ "setup_url": url }),
        None => serde_json::json!({ "redirect_url": "/auth/login/start" }),
    }))
}

/// Issues a fresh one-time setup URL for an already-provisioned pending user.
/// Called when the user returns to the invite page after the stored URL was consumed.
async fn refresh_setup_url(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    let pending = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
        &state.repo.db,
        &invitation.id,
    )
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::not_found("No pending account for this invitation"))?;

    let provisioner = state
        .provisioner
        .as_ref()
        .ok_or_else(|| AppError::failure("No identity provisioner configured"))?;

    let setup_url = provisioner
        .create_setup_url(&pending.sub)
        .await
        .map_err(|e| {
            tracing::warn!("refresh setup URL for invite {code}: {e}");
            AppError::failure(e.to_string())
        })?;

    tracing::info!(code, sub = %pending.sub, ?setup_url, "refreshed setup URL for pending invite");
    Ok(Json(match setup_url {
        Some(url) => serde_json::json!({ "setup_url": url.to_string() }),
        None => serde_json::json!({ "redirect_url": "/auth/login/start" }),
    }))
}

// --- Public invite flow (template-based, kept for reference) ---

#[derive(Deserialize)]
struct InviteQuery {
    error: Option<String>,
}

#[derive(Deserialize)]
struct RedeemForm {
    username: String,
    email: String,
    first_name: String,
    last_name: String,
}

async fn show_invite_form(
    State(state): State<AppState>,
    Path(code): Path<String>,
    axum::extract::Query(query): axum::extract::Query<InviteQuery>,
) -> AppResult<Response> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    if let Some(pending) = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
        &state.repo.db,
        &invitation.id,
    )
    .await
    .map_err(AppError::internal)?
    {
        let redirect_target = if let Some(stored_url) = pending.setup_url {
            stored_url
        } else if let Some(provisioner) = state.provisioner.as_ref() {
            provisioner
                .create_setup_url(&pending.sub)
                .await
                .map_err(|e| {
                    tracing::warn!("re-issue setup URL for invite {code}: {e}");
                    AppError::failure(e.to_string())
                })?
                .map(|u| u.to_string())
                .unwrap_or_else(|| "/auth/login/start".to_string())
        } else {
            "/auth/login/start".to_string()
        };
        return Ok(Redirect::to(&redirect_target).into_response());
    }

    let error = query.error.unwrap_or_default();
    render(InviteTemplate {
        title: "Create account",
        auth: AuthenticatedUser::default(),
        self_service_url: String::new(),
        code,
        error: error.clone(),
        has_error: !error.is_empty(),
    })
    .map(IntoResponse::into_response)
}

async fn redeem_invite(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Form(form): Form<RedeemForm>,
) -> AppResult<Response> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    if invitation
        .max_uses
        .is_some_and(|max| invitation.use_count >= max)
    {
        return Err(AppError::not_found("Invitation has been fully used"));
    }

    let provisioner = state
        .provisioner
        .as_ref()
        .ok_or_else(|| AppError::failure("No identity provisioner configured"))?;
    let display_name =
        invitation_display_name(&form.username, Some(&form.first_name), Some(&form.last_name));

    let provisioned = provisioner
        .create_user(CreateUserRequest {
            username: form.username.clone(),
            email: form.email.clone(),
            first_name: non_empty(form.first_name),
            last_name: non_empty(form.last_name),
        })
        .await
        .map_err(|e| {
            tracing::warn!("provisioner error for invite {code}: {e}");
            AppError::failure(e.to_string())
        })?;

    let setup_url_str = provisioned.setup_url.as_ref().map(|u| u.to_string());

    repos::pending_access::PendingAccessRepo::create(
        &state.repo.db,
        NewPendingAccess {
            sub: provisioned.external_id.clone(),
            invitation_id: invitation.id.clone(),
            is_admin: invitation.is_admin,
            grants: invitation
                .grants
                .iter()
                .map(|g| PendingAccessGrant {
                    product_id: g.product_id.clone(),
                    role: g.role.clone(),
                })
                .collect(),
            display_name: Some(display_name),
            setup_url: setup_url_str.clone(),
        },
    )
    .await
    .map_err(AppError::internal)?;

    repos::invitation::InvitationRepo::record_acceptance(
        &state.repo.db,
        &invitation.id,
        &form.username,
        &form.email,
    )
    .await
    .map_err(AppError::internal)?;

    let redirect = setup_url_str.unwrap_or_else(|| "/auth/login/start".to_string());

    Ok(Redirect::to(&redirect).into_response())
}

// --- Helpers ---

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

async fn build_products(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    grants: &[InvitationGrant],
) -> Vec<ProductGrant> {
    let mut out = Vec::with_capacity(grants.len());
    for grant in grants {
        let name = repos::product::ProductRepo::get_by_id(db, &grant.product_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.name)
            .unwrap_or_else(|| grant.product_id.clone());
        out.push(ProductGrant { name, role: role_label(&grant.role).to_string() });
    }
    out
}

/// Returns per-product subject, HTML, and text invite email templates from `product_settings`.
/// Returns `(None, None, None)` if no custom templates are configured for the product.
async fn product_email_templates(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    product_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(Some(settings)) =
        repos::product_settings::ProductSettingsRepo::get(db, product_id).await
    else {
        return (None, None, None);
    };
    (
        settings.email.invite_subject,
        settings.email.invite_html_template,
        settings.email.invite_text_template,
    )
}

/// Validates that a product-scoped token is not used to create grants
/// outside its own product or to create admin invitations.
fn authorize_api_token_grants(
    api_token: &ApiToken,
    body: &CreateInvitationRequest,
) -> AppResult<()> {
    if let Some(product_id) = api_token.product_id.as_deref()
        && (body.is_admin
            || body
                .grants
                .iter()
                .any(|grant| grant.product_id != product_id))
    {
        return Err(AppError::forbidden());
    }
    Ok(())
}
