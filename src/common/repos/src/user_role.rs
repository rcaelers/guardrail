use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::{RepoError, handle_surreal_error};
use data::user_role::{NewUserRole, UserRole};

pub struct UserRoleRepo {}

impl UserRoleRepo {
    pub async fn get_by_sub(
        db: &Surreal<Any>,
        sub: &str,
    ) -> Result<Option<UserRole>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id FROM user_access WHERE sub = $sub LIMIT 1",
            )
            .bind(("sub", sub.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<UserRole> = crate::take_many(&mut result, 0)?;
        Ok(rows.into_iter().next())
    }

    pub async fn create(db: &Surreal<Any>, user_role: NewUserRole) -> Result<String, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('user_access', $id) CONTENT {
                    sub: $sub,
                    roles: $roles,
                    created_at: time::now(),
                }",
            )
            .bind(("id", id.clone()))
            .bind(("sub", user_role.sub))
            .bind(("roles", serde_json::to_value(&user_role.roles)
                .map_err(|e| RepoError::DatabaseError(e.to_string()))?))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }
}
