use axum::{
    Form, Json, Router,
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use common::AuthenticatedUser;
use data::invitation::{Invitation, InvitationGrant, NewInvitation};
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/invitations", get(list_invitations).post(create_invitation))
        .route("/api/invitations/{id}", delete(revoke_invitation))
        .route("/invite/{code}", get(show_invite_form).post(redeem_invite))
}

// --- Admin API ---

#[derive(Deserialize)]
struct CreateInvitationRequest {
    is_admin: bool,
    grants: Vec<InvitationGrant>,
    expires_at: Option<DateTime<Utc>>,
    max_uses: Option<u32>,
}

async fn list_invitations(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Json<Vec<Invitation>>> {
    require_admin(&session).await?;
    let invitations = repos::invitation::InvitationRepo::get_all(
        &state.repo.db,
        common::QueryParams::default(),
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
    let admin = require_admin(&session).await?;
    let invitation = repos::invitation::InvitationRepo::create(
        &state.repo.db,
        NewInvitation {
            created_by: admin.id,
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

async fn revoke_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&session).await?;
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
    // 1. Validate invitation — must still be Active and within use limits
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

    // 2. Get provisioner
    let provisioner = state
        .provisioner
        .as_ref()
        .ok_or_else(|| AppError::failure("No identity provisioner configured"))?;

    // 3. Create user in the identity provider
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

    // 4. Persist access grants — applied to user_access on first OIDC login.
    //    The use_count is incremented there too, so the counter only moves
    //    after a passkey is successfully created.
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

    // 5. Send user to credential setup (passkey) or straight to OIDC login
    let redirect = provisioned
        .setup_url
        .map(|u| u.to_string())
        .unwrap_or_else(|| "/auth/login/start".to_string());

    Ok(Redirect::to(&redirect).into_response())
}

// --- Helpers ---

async fn require_admin(session: &Session) -> AppResult<AuthenticatedUser> {
    let user = session
        .get::<AuthenticatedUser>(AUTHENTICATED_USER_SESSION_KEY)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::forbidden())?;
    if !user.is_admin {
        return Err(AppError::forbidden());
    }
    Ok(user)
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}
