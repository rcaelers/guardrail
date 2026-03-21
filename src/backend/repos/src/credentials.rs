use sqlx::Postgres;

use crate::error::{RepoError, handle_sql_error};
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
                FROM core.credentials
                WHERE core.credentials.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_all_by_user_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        user_id: uuid::Uuid,
    ) -> Result<Vec<Credential>, RepoError> {
        sqlx::query_as!(
            Credential,
            r#"
                SELECT *
                FROM core.credentials
                WHERE core.credentials.user_id = $1
            "#,
            user_id
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        credentials: NewCredential,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.credentials
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
        .map_err(handle_sql_error)
    }

    pub async fn update_data(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
        data: serde_json::Value,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE core.credentials
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
        .map_err(handle_sql_error)
    }
}
