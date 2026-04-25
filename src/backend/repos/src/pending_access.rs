use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::{RepoError, handle_surreal_error};
use crate::record_key;
use data::pending_access::{NewPendingAccess, PendingAccess};

pub struct PendingAccessRepo {}

impl PendingAccessRepo {
    pub async fn get_by_sub(
        db: &Surreal<Any>,
        sub: &str,
    ) -> Result<Option<PendingAccess>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM pending_access WHERE sub = $sub LIMIT 1")
            .bind(("sub", sub.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<PendingAccess> = crate::take_many(&mut result, 0)?;
        Ok(rows.into_iter().next())
    }

    pub async fn get_by_invitation_id(
        db: &Surreal<Any>,
        invitation_id: &str,
    ) -> Result<Option<PendingAccess>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id FROM pending_access WHERE invitation_id = $invitation_id LIMIT 1",
            )
            .bind(("invitation_id", invitation_id.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<PendingAccess> = crate::take_many(&mut result, 0)?;
        Ok(rows.into_iter().next())
    }

    pub async fn create(db: &Surreal<Any>, pending: NewPendingAccess) -> Result<String, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('pending_access', $id) CONTENT {
                    sub:           $sub,
                    invitation_id: $invitation_id,
                    is_admin:      $is_admin,
                    grants:        $grants,
                    created_at:    time::now(),
                }",
            )
            .bind(("id", id.clone()))
            .bind(("sub", pending.sub))
            .bind(("invitation_id", pending.invitation_id))
            .bind(("is_admin", pending.is_admin))
            .bind((
                "grants",
                serde_json::to_value(&pending.grants)
                    .map_err(|e| RepoError::DatabaseError(e.to_string()))?,
            ))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    /// Write `user_access` rows for every grant, increment the invitation
    /// use-count (exhausting it if max_uses is reached), then delete the
    /// pending record.  Uses delete+create so re-runs are idempotent.
    pub async fn apply_and_delete(
        db: &Surreal<Any>,
        user_id: &str,
        pending: &PendingAccess,
    ) -> Result<(), RepoError> {
        let uid = record_key(user_id);

        for grant in &pending.grants {
            let pid = record_key(&grant.product_id);

            db.query(
                "DELETE user_access
                 WHERE user_id   = type::record('users',    $uid)
                   AND product_id = type::record('products', $pid)",
            )
            .bind(("uid", uid.clone()))
            .bind(("pid", pid.clone()))
            .await
            .map_err(handle_surreal_error)?;

            db.query(
                "CREATE user_access CONTENT {
                    user_id:    type::record('users',    $uid),
                    product_id: type::record('products', $pid),
                    role:       $role,
                    created_at: time::now(),
                    updated_at: time::now()
                }",
            )
            .bind(("uid", uid.clone()))
            .bind(("pid", pid.clone()))
            .bind(("role", grant.role.clone()))
            .await
            .map_err(handle_surreal_error)?;
        }

        crate::invitation::InvitationRepo::increment_and_maybe_exhaust(db, &pending.invitation_id)
            .await?;

        db.query("DELETE type::record('pending_access', $id)")
            .bind(("id", record_key(&pending.id)))
            .await
            .map_err(handle_surreal_error)?;

        Ok(())
    }
}
