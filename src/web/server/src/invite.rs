use axum::{
    Form, Json, Router,
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use common::AuthenticatedUser;
use data::invitation::{Invitation, InvitationGrant, NewInvitation, UpdateInvitation};
use data::pending_access::{NewPendingAccess, PendingAccessGrant};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::AuthSession,
    error::{AppError, AppResult},
    provisioner::CreateUserRequest,
    routes::render,
    templates::InviteTemplate,
};

const AUTHENTICATED_USER_SESSION_KEY: &str = "authenticated_user";

/// Invitation API routes, to be nested under /api/v1 in main.rs.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/invitations", get(list_invitations).post(create_invitation))
        .route("/invitations/{id}", put(update_invitation).delete(revoke_invitation))
}

/// Web routes for the invitation redemption flow.
pub fn web_router() -> Router<AppState> {
    Router::new()
        .route("/invite/{code}", get(show_invite_form).post(redeem_invite))
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
    let user = require_auth(&session).await?;
    let maintained_ids = get_maintained_product_ids(&state, &user.id).await?;
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
    Json(body): Json<CreateInvitationRequest>,
) -> AppResult<Json<Invitation>> {
    let user = require_auth(&session).await?;

    if !user.is_admin {
        let maintained_ids = get_maintained_product_ids(&state, &user.id).await?;
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

    let invitation = repos::invitation::InvitationRepo::create(
        &state.repo.db,
        NewInvitation {
            created_by: user.id,
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
    let user = require_auth(&session).await?;

    let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Invitation not found"))?;

    let (grants, is_admin) = if user.is_admin {
        (body.grants, body.is_admin)
    } else {
        let maintained_ids = get_maintained_product_ids(&state, &user.id).await?;

        // Must have at least one overlap to be allowed to edit at all.
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

        // Merge: keep grants for products the user doesn't maintain unchanged.
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
    let user = require_auth(&session).await?;

    if !user.is_admin {
        let invitation = repos::invitation::InvitationRepo::get_by_id(&state.repo.db, &id)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::not_found("Invitation not found"))?;

        let maintained_ids = get_maintained_product_ids(&state, &user.id).await?;
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

// --- Public invite flow ---

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

    // If a pending_access already exists for this invitation the user previously
    // started setup but aborted (e.g. closed the passkey page).  Re-issue a
    // fresh setup URL so they can continue without filling in the form again.
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

    // Persist access grants — applied to user_access on first OIDC login.
    // use_count is incremented there, so the counter only moves after a
    // passkey is successfully created.
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

async fn require_auth(session: &Session) -> AppResult<AuthenticatedUser> {
    session
        .get::<AuthenticatedUser>(AUTHENTICATED_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(AppError::forbidden)
}

/// Returns product IDs (plain UUID strings) where the user has the maintainer role.
async fn get_maintained_product_ids(state: &AppState, user_id: &str) -> AppResult<Vec<String>> {
    let uid = repos::record_key(user_id);
    let mut result = state
        .repo
        .db
        .query(
            "SELECT meta::id(product_id) as pid FROM user_access
             WHERE user_id = type::record('users', $uid)
               AND role = 'maintainer'",
        )
        .bind(("uid", uid))
        .await
        .map_err(AppError::internal)?;

    let rows: Vec<serde_json::Value> = result.take(0).map_err(AppError::internal)?;
    let ids = rows
        .into_iter()
        .filter_map(|v| v.get("pid").and_then(|p| p.as_str()).map(str::to_owned))
        .collect();
    Ok(ids)
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}
