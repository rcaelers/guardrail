use chrono::Utc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::{RepoError, handle_surreal_error};
use data::credentials::{Credential, NewCredential};

pub struct CredentialsRepo {}

impl CredentialsRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: uuid::Uuid,
    ) -> Result<Option<Credential>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('credentials', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_all_by_user_id(
        db: &Surreal<Any>,
        user_id: uuid::Uuid,
    ) -> Result<Vec<Credential>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM credentials WHERE user_id = $user_id")
            .bind(("user_id", user_id))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(
        db: &Surreal<Any>,
        credentials: NewCredential,
    ) -> Result<uuid::Uuid, RepoError> {
        let id = uuid::Uuid::new_v4();
        let now = Utc::now();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('credentials', $id) CONTENT {
                user_id: $user_id,
                name: 'fixme',
                data: $data,
                last_used: $last_used,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.to_string()))
            .bind(("user_id", credentials.user_id))
            .bind(("data", credentials.data.clone()))
            .bind(("last_used", now))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update_data(
        db: &Surreal<Any>,
        id: uuid::Uuid,
        data: serde_json::Value,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        let now = Utc::now();
        let mut result = db
            .query(
                "UPDATE type::record('credentials', $id) SET
                data = $data,
                last_used = $last_used,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", id.to_string()))
            .bind(("data", data.clone()))
            .bind(("last_used", now))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows.first().and_then(|r| {
            r.get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
        }))
    }
}
