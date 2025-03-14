use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct User {
    pub id: uuid::Uuid,
    pub username: String,
    pub is_admin: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewUser {
    pub username: String,
    pub is_admin: bool,
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{NewUser, User};
    use crate::{Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};
    use std::collections::HashSet;

    pub struct UserRepo {}

    impl UserRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<User>, RepoError> {
            let row = sqlx::query_as!(
                User,
                r#"
                SELECT *
                FROM guardrail.users
                WHERE guardrail.users.id = $1
            "#,
                id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve user {id}: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_by_name(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            email: &str,
        ) -> Result<Option<User>, RepoError> {
            let row = sqlx::query_as!(
                User,
                r#"
                SELECT *
                FROM guardrail.users
                WHERE guardrail.users.username = $1
            "#,
                email.to_string()
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve user by email: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(row)
        }

        pub async fn get_all_names(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<HashSet<String>, RepoError> {
            let rows = sqlx::query!(
                r#"
                SELECT username
                FROM guardrail.users
            "#
            )
            .fetch_all(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to retrieve all user names: {err}");
                RepoError::DatabaseError(message)
            })?
            .into_iter()
            .map(|row| row.username)
            .collect::<HashSet<String>>();

            Ok(rows)
        }

        pub async fn get_all(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            params: crate::QueryParams,
        ) -> Result<Vec<User>, RepoError> {
            let mut builder = QueryBuilder::new("SELECT * from guardrail.products");
            Repo::build_query(
                &mut builder,
                &params,
                &["id", "username", "created_at", "updated_at"],
                &["username"],
            )?;

            let query = builder.build_query_as();

            let rows = query.fetch_all(executor).await.map_err(|err| {
                let message = format!("Failed to retrieve all products: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(rows)
        }

        pub async fn create_with_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
            username: &str,
        ) -> Result<uuid::Uuid, RepoError> {
            let user_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.users
                  (
                    id,
                    username,
                    is_admin
                  )
                VALUES ($1, $2, false)
                RETURNING
                  id
            "#,
                id,
                username,
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create user: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(user_id)
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user: NewUser,
        ) -> Result<uuid::Uuid, RepoError> {
            let user_id = sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.users
                  (
                    username,
                    is_admin
                  )
                VALUES ($1, false)
                RETURNING
                  id
            "#,
                user.username,
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to create user: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(user_id)
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            user: User,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            let id = sqlx::query_scalar!(
                r#"
                UPDATE guardrail.users
                SET username = $1, is_admin = $2
                WHERE id = $3
                RETURNING id
            "#,
                user.username,
                user.is_admin,
                user.id,
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to update user: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(id)
        }

        pub async fn remove(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<(), RepoError> {
            sqlx::query!(
                r#"
                DELETE FROM guardrail.users
                WHERE id = $1
            "#,
                id
            )
            .execute(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to users version: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(())
        }

        pub async fn count(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<i64, RepoError> {
            let count = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*)
                FROM guardrail.users
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                let message = format!("Failed to count users: {err}");
                RepoError::DatabaseError(message)
            })?;

            Ok(count.unwrap_or(0))
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
