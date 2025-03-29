use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct ApiToken {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub description: String,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub last_used_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewApiToken {
    pub description: String,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub expires_at: Option<NaiveDateTime>,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{ApiToken, NewApiToken};
    use crate::error::RepoError;
    use chrono::Utc;
    use sqlx::Postgres;
    use uuid::Uuid;

    pub struct ApiTokenRepo {}

    impl ApiTokenRepo {
        /// Get an API token by its ID
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: Uuid,
        ) -> Result<Option<ApiToken>, RepoError> {
            let row = sqlx::query_as!(
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
            .map_err(|err| {
                let message = format!("Failed to retrieve API token {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        /// Get an API token by its token hash
        pub async fn get_by_token_hash(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            token_hash: &str,
        ) -> Result<Option<ApiToken>, RepoError> {
            let row = sqlx::query_as!(
                ApiToken,
                r#"
                SELECT *
                FROM guardrail.api_tokens
                WHERE guardrail.api_tokens.token_hash = $1
                AND (guardrail.api_tokens.expires_at IS NULL OR guardrail.api_tokens.expires_at > now())
                AND guardrail.api_tokens.is_active = true
            "#,
                token_hash
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve API token by token hash: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        /// Update the last_used_at field for a token
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
            .map_err(|err| {
                let message = format!("Failed to update last_used_at for token {token_id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        /// Get all API tokens for a product
        pub async fn get_by_product_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            product_id: Uuid,
        ) -> Result<Vec<ApiToken>, RepoError> {
            let rows = sqlx::query_as!(
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
            .map_err(|err| {
                let message =
                    format!("Failed to retrieve API tokens for product {product_id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        /// Get all API tokens for a user
        pub async fn get_by_user_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user_id: Uuid,
        ) -> Result<Vec<ApiToken>, RepoError> {
            let rows = sqlx::query_as!(
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
            .map_err(|err| {
                let message = format!("Failed to retrieve API tokens for user {user_id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        /// Create a new API token
        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            new_token: NewApiToken,
        ) -> Result<Uuid, RepoError> {
            let token_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.api_tokens
                  (
                    description,
                    token_hash,
                    product_id,
                    user_id,
                    entitlements,
                    expires_at,
                    is_active
                  )
                VALUES ($1, $2, $3, $4, $5, $6, true)
                RETURNING
                  id
            "#,
                new_token.description,
                new_token.token_hash,
                new_token.product_id,
                new_token.user_id,
                &new_token.entitlements as &[String],
                new_token.expires_at,
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create API token: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(token_id)
        }

        /// Update an API token
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
            .map_err(|err| {
                let message = format!("Failed to update API token {}: {err}", token.id);
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        /// Revoke/deactivate an API token
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
            .map_err(|err| {
                let message = format!("Failed to revoke API token {token_id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        /// Delete an API token
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
            .map_err(|err| {
                let message = format!("Failed to delete API token {token_id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        /// Check if a token has a specific entitlement
        pub fn has_entitlement(token: &ApiToken, required_entitlement: &str) -> bool {
            if !token.is_active {
                return false;
            }

            // Check if token is expired
            if let Some(expires_at) = token.expires_at {
                let now = Utc::now().naive_utc();
                if expires_at < now {
                    return false;
                }
            }

            // Direct entitlement match
            if token
                .entitlements
                .contains(&required_entitlement.to_string())
            {
                return true;
            }

            // Token entitlement grants access to all functionality
            if token.entitlements.contains(&"token".to_string()) {
                return true;
            }

            false
        }

        /// Get all API tokens
        pub async fn get_all(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<Vec<ApiToken>, RepoError> {
            let rows = sqlx::query_as!(
                ApiToken,
                r#"
                SELECT *
                FROM guardrail.api_tokens
                ORDER BY created_at DESC
            "#
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve all API tokens: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
