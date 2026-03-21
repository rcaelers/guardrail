use sqlx::{Postgres, QueryBuilder};
use std::collections::HashSet;

use crate::{
    Repo,
    error::{RepoError, handle_sql_error},
};
use common::QueryParams;
use data::user::{NewUser, User};

pub struct UserRepo {}

impl UserRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<User>, RepoError> {
        sqlx::query_as!(
            User,
            r#"
                SELECT *
                FROM core.users
                WHERE core.users.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_by_name(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        username: &str,
    ) -> Result<Option<User>, RepoError> {
        sqlx::query_as!(
            User,
            r#"
                SELECT *
                FROM core.users
                WHERE core.users.username = $1
            "#,
            username.to_string()
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_all_names(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<HashSet<String>, RepoError> {
        sqlx::query!(
            r#"
                SELECT username
                FROM core.users
            "#
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
        .map(|rows| {
            rows.into_iter()
                .map(|row| row.username)
                .collect::<HashSet<String>>()
        })
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<User>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from core.users");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "username", "created_at", "updated_at"],
            &["username"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(handle_sql_error)
    }

    pub async fn create_with_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
        username: &str,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.users
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
        .map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        user: NewUser,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.users
                  (
                    username,
                    is_admin
                  )
                VALUES ($1, $2)
                RETURNING
                  id
            "#,
            user.username,
            user.is_admin,
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        user: User,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE core.users
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
        .map_err(handle_sql_error)
    }

    pub async fn remove(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM core.users
                WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)
        .map(|_| ())
    }

    pub async fn count(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<i64, RepoError> {
        sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM core.users
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
