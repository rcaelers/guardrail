use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
    record_key,
};
use common::QueryParams;
use data::invitation::{Invitation, InvitationStatus, NewInvitation, UpdateInvitation};

pub struct InvitationRepo {}

impl InvitationRepo {
    /// Return invitations visible to a user:
    /// admins get everything; others see invitations they created OR where any
    /// grant's product_id is in `maintained_product_ids`.
    pub async fn get_for_user(
        db: &Surreal<Any>,
        user_id: &str,
        is_admin: bool,
        maintained_product_ids: &[String],
    ) -> Result<Vec<Invitation>, RepoError> {
        if is_admin {
            let mut result = db
                .query("SELECT *, meta::id(id) as id FROM invitations ORDER BY created_at DESC")
                .await
                .map_err(handle_surreal_error)?;
            return crate::take_many(&mut result, 0);
        }
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id FROM invitations
                 WHERE created_by = $user_id
                    OR grants[WHERE product_id INSIDE $product_ids] != []
                 ORDER BY created_at DESC",
            )
            .bind(("user_id", user_id.to_owned()))
            .bind(("product_ids", maintained_product_ids.to_vec()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn update(
        db: &Surreal<Any>,
        id: &str,
        update: UpdateInvitation,
    ) -> Result<Option<Invitation>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('invitations', $id) SET
                    expires_at = $expires_at,
                    max_uses   = $max_uses,
                    is_admin   = $is_admin,
                    grants     = $grants,
                    updated_at = time::now()
                 RETURN *, meta::id(id) as id",
            )
            .bind(("id", record_key(id)))
            .bind(("expires_at", update.expires_at))
            .bind(("max_uses", update.max_uses))
            .bind(("is_admin", update.is_admin))
            .bind((
                "grants",
                serde_json::to_value(&update.grants)
                    .map_err(|e| RepoError::DatabaseError(e.to_string()))?,
            ))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<Invitation>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('invitations', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_code(
        db: &Surreal<Any>,
        code: &str,
    ) -> Result<Option<Invitation>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id FROM invitations
                 WHERE code = $code
                   AND status = 'Active'
                   AND (expires_at IS NONE OR expires_at > time::now())
                 LIMIT 1",
            )
            .bind(("code", code.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<Invitation> = crate::take_many(&mut result, 0)?;
        Ok(rows.into_iter().next())
    }

    pub async fn get_all(
        db: &Surreal<Any>,
        params: QueryParams,
    ) -> Result<Vec<Invitation>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "code", "status", "created_at", "updated_at"],
            &[],
        )?;
        let query = format!("SELECT *, meta::id(id) as id FROM invitations{suffix}");
        let mut builder = db.query(&query);
        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }
        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(
        db: &Surreal<Any>,
        invitation: NewInvitation,
    ) -> Result<Invitation, RepoError> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let code = uuid::Uuid::new_v4().simple().to_string();
        let mut result = db
            .query(
                "CREATE type::record('invitations', $id) CONTENT {
                    code:       $code,
                    created_by: $created_by,
                    expires_at: $expires_at,
                    max_uses:   $max_uses,
                    use_count:  0,
                    is_admin:   $is_admin,
                    grants:     $grants,
                    status:     'Active',
                    created_at: time::now(),
                    updated_at: time::now(),
                } RETURN *, meta::id(id) as id",
            )
            .bind(("id", id))
            .bind(("code", code))
            .bind(("created_by", invitation.created_by))
            .bind(("expires_at", invitation.expires_at))
            .bind(("max_uses", invitation.max_uses))
            .bind(("is_admin", invitation.is_admin))
            .bind((
                "grants",
                serde_json::to_value(&invitation.grants)
                    .map_err(|e| RepoError::DatabaseError(e.to_string()))?,
            ))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("invitation not created".into()))
    }

    pub async fn increment_use_count(
        db: &Surreal<Any>,
        id: &str,
    ) -> Result<Option<Invitation>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('invitations', $id) SET
                    use_count = use_count + 1,
                    updated_at = time::now()
                 RETURN *, meta::id(id) as id",
            )
            .bind(("id", record_key(id)))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn set_status(
        db: &Surreal<Any>,
        id: &str,
        status: InvitationStatus,
    ) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('invitations', $id) SET
                status = $status,
                updated_at = time::now()",
        )
        .bind(("id", record_key(id)))
        .bind(("status", format!("{status:?}")))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }

    /// Atomically increments use_count.  If max_uses is now reached, sets
    /// status to Exhausted.  Silently succeeds if the record is missing (e.g.
    /// the invitation was deleted concurrently).
    pub async fn increment_and_maybe_exhaust(db: &Surreal<Any>, id: &str) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('invitations', $id) SET
                use_count  = use_count + 1,
                status     = IF max_uses != NONE AND (use_count + 1) >= max_uses THEN 'Exhausted' ELSE status END,
                updated_at = time::now()",
        )
        .bind(("id", record_key(id)))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn revoke(db: &Surreal<Any>, id: impl ToString) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('invitations', $id) SET
                status = 'Revoked',
                updated_at = time::now()",
        )
        .bind(("id", record_key(id.to_string())))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }
}
