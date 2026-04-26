use axum::{
    Form, Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use data::api_token::{ApiToken, ENTITLEMENT_INVITATION_CREATE};
use data::invitation::{Invitation, InvitationGrant, NewInvitation, UpdateInvitation};
use data::pending_access::{NewPendingAccess, PendingAccessGrant};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    access,
    access::Principal,
    auth::AuthSession,
    error::{AppError, AppResult},
    provisioner::CreateUserRequest,
    routes::render,
    templates::InviteTemplate,
};

/// Invitation API routes, to be nested under /api/v1 in main.rs.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/invitations", get(list_invitations).post(create_invitation))
        .route("/invitations/{id}", put(update_invitation).delete(revoke_invitation))
        .route("/invitations/redeem/{code}", get(get_invite_info).post(redeem_invite_json))
}

/// Web routes for the invitation redemption flow.
pub fn web_router() -> Router<AppState> {
    Router::new().route("/invite/{code}", get(show_invite_form).post(redeem_invite))
}

// --- Invitation API ---

#[derive(Deserialize)]
struct CreateInvitationRequest {
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
    let maintained_ids = access::get_maintained_product_ids(&state.repo.db, &user.id).await?;
    let invitations = repos::invitation::InvitationRepo::get_for_user(
        &state.repo.db,
        &user.id,
        user.is_admin,
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
    let principal =
        access::require_session_or_entitlement(&session, &headers, &state.repo.db, ENTITLEMENT_INVITATION_CREATE)
            .await?;

    let created_by = match &principal {
        Principal::Token(token) => {
            authorize_api_token_grants(token, &body)?;
            format!("api-token:{}", token.id)
        }
        Principal::User(user) => {
            if !user.is_admin {
                let maintained_ids =
                    access::get_maintained_product_ids(&state.repo.db, &user.id).await?;
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
            user.id.clone()
        }
    };

    let invitation = repos::invitation::InvitationRepo::create(
        &state.repo.db,
        NewInvitation {
            created_by,
            expires_at: body.expires_at,
            max_uses: body.max_uses,
            is_admin: body.is_admin,
            grants: body.grants,
        },
    )
    .await
    .map_err(AppError::internal)?;
    Ok(Json(invitation))
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

    let (grants, is_admin) = if user.is_admin {
        (body.grants, body.is_admin)
    } else {
        let maintained_ids = access::get_maintained_product_ids(&state.repo.db, &user.id).await?;

        let has_overlap = invitation
            .grants
            .iter()
            .any(|g| maintained_ids.contains(&g.product_id))
            || invitation.created_by == user.id;
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

    if !user.is_admin {
        let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::not_found("Invitation not found"))?;

        let maintained_ids = access::get_maintained_product_ids(&state.repo.db, &user.id).await?;
        let can_revoke = invitation.created_by == user.id
            || invitation
                .grants
                .iter()
                .any(|g| maintained_ids.contains(&g.product_id));
        if !can_revoke {
            return Err(AppError::forbidden());
        }
    }

    repos::invitation::InvitationRepo::revoke(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(serde_json::json!({ "status": "revoked" })))
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

    if let Some(provisioner) = state.provisioner.as_ref() {
        if let Some(pending) = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
            &state.repo.db,
            &invitation.id,
        )
        .await
        .map_err(AppError::internal)?
        {
            let setup_url = provisioner
                .create_setup_url(&pending.sub)
                .await
                .map_err(|e| {
                    tracing::warn!("re-issue setup URL for invite {code}: {e}");
                    AppError::failure(e.to_string())
                })?;
            return Ok(Json(
                serde_json::json!({ "valid": true, "redirect_url": setup_url.to_string() }),
            ));
        }
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

async fn redeem_invite_json(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(body): Json<RedeemJsonRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let invitation = repos::invitation::InvitationRepo::get_by_code(&state.repo.db, &code)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found or has expired"))?;

    if invitation.max_uses.is_some_and(|max| invitation.use_count >= max) {
        return Err(AppError::not_found("Invitation has been fully used"));
    }

    let provisioner = state
        .provisioner
        .as_ref()
        .ok_or_else(|| AppError::failure("No identity provisioner configured"))?;

    if let Some(pending) = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
        &state.repo.db,
        &invitation.id,
    )
    .await
    .map_err(AppError::internal)?
    {
        let setup_url = provisioner
            .create_setup_url(&pending.sub)
            .await
            .map_err(|e| {
                tracing::warn!("re-issue setup URL for invite {code}: {e}");
                AppError::failure(e.to_string())
            })?;
        return Ok(Json(
            serde_json::json!({ "redirect_url": setup_url.to_string() }),
        ));
    }

    let provisioned = provisioner
        .create_user(CreateUserRequest {
            username: body.username.clone(),
            email: body.email.clone(),
            first_name: body.first_name,
            last_name: body.last_name,
        })
        .await
        .map_err(|e| {
            tracing::warn!("provisioner error for invite {code}: {e}");
            AppError::failure(e.to_string())
        })?;

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
        },
    )
    .await
    .map_err(AppError::internal)?;

    let redirect_url = provisioned
        .setup_url
        .map(|u| u.to_string())
        .unwrap_or_else(|| "/auth/login/start".to_string());

    Ok(Json(serde_json::json!({ "redirect_url": redirect_url })))
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

    if let Some(provisioner) = state.provisioner.as_ref() {
        if let Some(pending) = repos::pending_access::PendingAccessRepo::get_by_invitation_id(
            &state.repo.db,
            &invitation.id,
        )
        .await
        .map_err(AppError::internal)?
        {
            let setup_url = provisioner
                .create_setup_url(&pending.sub)
                .await
                .map_err(|e| {
                    tracing::warn!("re-issue setup URL for invite {code}: {e}");
                    AppError::failure(e.to_string())
                })?;
            return Ok(Redirect::to(setup_url.as_str()).into_response());
        }
    }

    let error = query.error.unwrap_or_default();
    render(InviteTemplate {
        title: "Create account",
        app_name: &state.settings.auth.name,
        auth: AuthSession::default(),
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
        },
    )
    .await
    .map_err(AppError::internal)?;

    let redirect = provisioned
        .setup_url
        .map(|u| u.to_string())
        .unwrap_or_else(|| "/auth/login/start".to_string());

    Ok(Redirect::to(&redirect).into_response())
}

// --- Helpers ---

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Validates that a product-scoped token is not used to create grants
/// outside its own product or to create admin invitations.
fn authorize_api_token_grants(
    api_token: &ApiToken,
    body: &CreateInvitationRequest,
) -> AppResult<()> {
    if let Some(product_id) = api_token.product_id.as_deref() {
        if body.is_admin
            || body
                .grants
                .iter()
                .any(|grant| grant.product_id != product_id)
        {
            return Err(AppError::forbidden());
        }
    }
    Ok(())
}
