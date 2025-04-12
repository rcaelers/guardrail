use sqlx::Postgres;
use tracing::error;

use crate::error::RepoError;
use data::credentials::{Credential, NewCredential};

pub struct CredentialsRepo {}

impl CredentialsRepo {
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

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        credentials: NewCredential,
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
            credentials.user_id,
            credentials.data,
            chrono::Utc::now().naive_utc(),
        )
        .fetch_one(executor)
        .await
        .map_err(|err| {
            error!("Failed to create credential for user {}: {}", credentials.user_id, err);
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
