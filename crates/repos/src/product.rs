use sqlx::{Postgres, QueryBuilder};
use std::collections::HashSet;

use crate::{
    Repo,
    error::{RepoError, handle_sql_error},
};
use common::QueryParams;
use data::product::{NewProduct, Product};

pub struct ProductRepo {}

impl ProductRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<Product>, RepoError> {
        sqlx::query_as!(
            Product,
            r#"
                SELECT *
                FROM core.products
                WHERE core.products.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_by_name(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        name: &str,
    ) -> Result<Option<Product>, RepoError> {
        sqlx::query_as!(
            Product,
            r#"
                SELECT *
                FROM core.products
                WHERE core.products.name = $1
            "#,
            name.to_string()
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
                SELECT name
                FROM core.products
            "#
        )
        .fetch_all(executor)
        .await
        .map_err(handle_sql_error)
        .map(|rows| {
            rows.into_iter()
                .map(|row| row.name)
                .collect::<HashSet<String>>()
        })
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<Product>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from core.products");
        Repo::build_query(
            &mut builder,
            &params,
            &[
                "id",
                "name",
                "description",
                "accepting_crashes",
                "created_at",
                "updated_at",
            ],
            &["name", "description"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        product: NewProduct,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.products
                  (
                    name,
                    description
                  )
                VALUES ($1, $2)
                RETURNING
                  id
            "#,
            product.name,
            product.description
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        product: Product,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE core.products
                SET name = $1, description = $2, accepting_crashes = $3
                WHERE id = $4
                RETURNING id
            "#,
            product.name,
            product.description,
            product.accepting_crashes,
            product.id,
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
                DELETE FROM core.products
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
                FROM core.products
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
