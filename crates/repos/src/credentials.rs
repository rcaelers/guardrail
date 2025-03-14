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

    pub struct CredentialRepo {}

    impl CredentialRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Credential>, RepoError> {
            let row = sqlx::query_as!(
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
                let message = format!("Failed to retrieve credential {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_all_by_user_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user_id: uuid::Uuid,
        ) -> Result<Vec<Credential>, RepoError> {
            let rows = sqlx::query_as!(
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
                let message = format!("Failed to retrieve credential by user_id: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn get_all_by_name(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            name: &str,
        ) -> Result<Vec<Credential>, RepoError> {
            let rows = sqlx::query_as!(
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
                let message = format!("Failed to retrieve credential by name: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user_id: uuid::Uuid,
            data: serde_json::Value,
        ) -> Result<uuid::Uuid, RepoError> {
            let credential_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.credentials
                  (
                    user_id,
                    name,
                    data
                  )
                VALUES ($1, 'fixme', $2)
                RETURNING
                  id
            "#,
                user_id,
                data
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create credential: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(credential_id)
        }

        pub async fn update_data(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
            data: serde_json::Value,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
                r#"
                UPDATE guardrail.credentials
                SET data = $1
                WHERE id = $2
                RETURNING id
            "#,
                data,
                id,
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to update credential: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(id)
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
