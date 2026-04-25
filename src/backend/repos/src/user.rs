use std::collections::HashSet;

use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
    record_key,
};
use common::QueryParams;
use data::user::{NewUser, User};

pub struct UserRepo {}

impl UserRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<User>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('users', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_name(db: &Surreal<Any>, username: &str) -> Result<Option<User>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM users WHERE username = $username LIMIT 1")
            .bind(("username", username.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let users: Vec<User> = crate::take_many(&mut result, 0)?;
        Ok(users.into_iter().next())
    }

    pub async fn get_all_names(db: &Surreal<Any>) -> Result<HashSet<String>, RepoError> {
        let mut result = db
            .query("SELECT username FROM users")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.get("username").and_then(|v| v.as_str()).map(String::from))
            .collect())
    }

    pub async fn get_all(db: &Surreal<Any>, params: QueryParams) -> Result<Vec<User>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "username", "created_at", "updated_at"],
            &["username"],
        )?;

        let query = format!("SELECT *, meta::id(id) as id FROM users{suffix}");
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create_with_id(
        db: &Surreal<Any>,
        id: impl ToString,
        username: &str,
    ) -> Result<String, RepoError> {
        let id = record_key(id.to_string());
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('users', $id) CONTENT {
                username: $username,
                email: $email,
                is_admin: false,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.clone()))
            .bind(("username", username.to_owned()))
            .bind(("email", format!("{username}@test.local")))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn create(db: &Surreal<Any>, user: NewUser) -> Result<String, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let email = user
            .email
            .unwrap_or_else(|| format!("{}@test.local", user.username));
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('users', $id) CONTENT {
                username: $username,
                email: $email,
                is_admin: $is_admin,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.clone()))
            .bind(("username", user.username.clone()))
            .bind(("email", email))
            .bind(("is_admin", user.is_admin))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(db: &Surreal<Any>, user: User) -> Result<Option<String>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('users', $id) SET
                username = $username,
                is_admin = $is_admin,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", user.id.clone()))
            .bind(("username", user.username.clone()))
            .bind(("is_admin", user.is_admin))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string))
    }

    pub async fn remove(db: &Surreal<Any>, id: impl ToString) -> Result<(), RepoError> {
        db.query("DELETE type::record('users', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(db: &Surreal<Any>) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM users GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }
}
