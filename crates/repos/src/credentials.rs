use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Credential {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_used: NaiveDateTime,
    pub data: serde_json::Value,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::Credential;
    use crate::error::RepoError;
    use sqlx::Postgres;
    use tracing::error;

    pub struct CredentialRepo {}

    impl CredentialRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Credential>, RepoError> {
            sqlx::query_as!(
                Credential,
                r#"
                SELECT *
                FROM guardrail.credentials
                WHERE guardrail.credentials.id = $1
            "#,
                id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve credential {id}: {err}");
                RepoError::DatabaseError("Failed to retrieve credential".to_string())
            })
        }

        pub async fn get_all_by_user_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user_id: uuid::Uuid,
        ) -> Result<Vec<Credential>, RepoError> {
            sqlx::query_as!(
                Credential,
                r#"
                SELECT *
                FROM guardrail.credentials
                WHERE guardrail.credentials.user_id = $1
            "#,
                user_id
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve credentials for user {user_id}: {err}");
                RepoError::DatabaseError("Failed to retrieve credentials for user".to_string())
            })
        }

        pub async fn get_all_by_name(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            name: &str,
        ) -> Result<Vec<Credential>, RepoError> {
            sqlx::query_as!(
                Credential,
                r#"
                SELECT *
                FROM guardrail.credentials
                WHERE guardrail.credentials.name = $1
            "#,
                name
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve credentials by name {name}: {err}");
                RepoError::DatabaseError("Failed to retrieve credentials by name".to_string())
            })
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user_id: uuid::Uuid,
            data: serde_json::Value,
        ) -> Result<uuid::Uuid, RepoError> {
            sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.credentials
                  (
                    user_id,
                    name,
                    data,
                    last_used
                  )
                VALUES ($1, 'fixme', $2, $3)
                RETURNING
                  id
            "#,
                user_id,
                data,
                chrono::Utc::now().naive_utc(),
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                error!("Failed to create credential for user {user_id}: {err}");
                RepoError::DatabaseError("Failed to create credential".to_string())
            })
        }

        pub async fn update_data(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
            data: serde_json::Value,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            sqlx::query_scalar!(
                r#"
                UPDATE guardrail.credentials
                SET data = $1, last_used = $2
                WHERE id = $3
                RETURNING id
            "#,
                data,
                chrono::Utc::now().naive_utc(),
                id,
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                error!("Failed to update credential {id}: {err}");
                RepoError::DatabaseError("Failed to update credential".to_string())
            })
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
