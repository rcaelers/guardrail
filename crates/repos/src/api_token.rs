use sqlx::Postgres;
use uuid::Uuid;

use crate::error::{RepoError, handle_sql_error};
use data::api_token::{ApiToken, NewApiToken};

pub struct ApiTokenRepo {}

impl ApiTokenRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: Uuid,
    ) -> Result<Option<ApiToken>, RepoError> {
        sqlx::query_as!(
            ApiToken,
            r#"
                SELECT *
                FROM guardrail.api_tokens
                WHERE guardrail.api_tokens.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_by_token_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        token_id: Uuid,
    ) -> Result<Option<ApiToken>, RepoError> {
        sqlx::query_as!(
            ApiToken,
            r#"
                SELECT *
                FROM guardrail.api_tokens
                WHERE guardrail.api_tokens.token_id = $1
            "#,
            token_id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update_last_used(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        token_id: Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                UPDATE guardrail.api_tokens
                SET last_used_at = now()
                WHERE id = $1
            "#,
            token_id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)
        .map(|_| ())
    }

    pub async fn get_by_product_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        product_id: Uuid,
    ) -> Result<Vec<ApiToken>, RepoError> {
        sqlx::query_as!(
            ApiToken,
            r#"
                SELECT *
                FROM guardrail.api_tokens
                WHERE guardrail.api_tokens.product_id = $1
                ORDER BY created_at DESC
            "#,
            product_id
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_by_user_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        user_id: Uuid,
    ) -> Result<Vec<ApiToken>, RepoError> {
        sqlx::query_as!(
            ApiToken,
            r#"
                SELECT *
                FROM guardrail.api_tokens
                WHERE guardrail.api_tokens.user_id = $1
                ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        new_token: NewApiToken,
    ) -> Result<Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO guardrail.api_tokens
                  (
                    description,
                    token_id,
                    token_hash,
                    product_id,
                    user_id,
                    entitlements,
                    expires_at,
                    is_active
                  )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING
                  id
            "#,
            new_token.description,
            new_token.token_id,
            new_token.token_hash,
            new_token.product_id,
            new_token.user_id,
            &new_token.entitlements as &[String],
            new_token.expires_at,
            new_token.is_active,
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        token: ApiToken,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                UPDATE guardrail.api_tokens
                SET
                    description = $1,
                    entitlements = $2,
                    expires_at = $3,
                    is_active = $4
                WHERE id = $5
            "#,
            token.description,
            &token.entitlements as &[String],
            token.expires_at,
            token.is_active,
            token.id,
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)
        .map(|_| ())
    }

    pub async fn revoke(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        token_id: Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                UPDATE guardrail.api_tokens
                SET is_active = false
                WHERE id = $1
            "#,
            token_id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)?;
        Ok(())
    }

    pub async fn delete(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        token_id: Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM guardrail.api_tokens
                WHERE id = $1
            "#,
            token_id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)?;

        Ok(())
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<Vec<ApiToken>, RepoError> {
        sqlx::query_as!(
            ApiToken,
            r#"
                SELECT *
                FROM guardrail.api_tokens
            "#
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
    }
}
