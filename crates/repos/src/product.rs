use sqlx::{Postgres, QueryBuilder};
use std::collections::HashSet;
use tracing::error;

use common::QueryParams;
use crate::{Repo, error::RepoError};
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
                FROM guardrail.products
                WHERE guardrail.products.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(|err| {
            error!("Failed to retrieve product {id}: {err}");
            RepoError::DatabaseError("Failed to retrieve product".to_string())
        })
    }

    pub async fn get_by_name(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        name: &str,
    ) -> Result<Option<Product>, RepoError> {
        sqlx::query_as!(
            Product,
            r#"
                SELECT *
                FROM guardrail.products
                WHERE guardrail.products.name = $1
            "#,
            name.to_string()
        )
        .fetch_optional(executor)
        .await
        .map_err(|err| {
            error!("Failed to retrieve product by name {name}: {err}");
            RepoError::DatabaseError("Failed to retrieve product by name".to_string())
        })
    }

    pub async fn get_all_names(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<HashSet<String>, RepoError> {
        sqlx::query!(
            r#"
                SELECT name
                FROM guardrail.products
            "#
        )
        .fetch_all(executor)
        .await
        .map_err(|err| {
            error!("Failed to retrieve all product names: {err}");
            RepoError::DatabaseError("Failed to retrieve all product names".to_string())
        })
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
        let mut builder = QueryBuilder::new("SELECT * from guardrail.products");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "name", "description", "created_at", "updated_at"],
            &["name", "description"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(|err| {
            error!("Failed to retrieve all products: {err}");
            RepoError::DatabaseError("Failed to retrieve products".to_string())
        })
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        product: NewProduct,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO guardrail.products
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
        .map_err(|err| {
            error!("Failed to create product: {err}");
            RepoError::DatabaseError("Failed to create product".to_string())
        })
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        product: Product,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE guardrail.products
                SET name = $1, description = $2
                WHERE id = $3
                RETURNING id
            "#,
            product.name,
            product.description,
            product.id,
        )
        .fetch_optional(executor)
        .await
        .map_err(|err| {
            error!("Failed to update product {}: {err}", product.id);
            RepoError::DatabaseError("Failed to update product".to_string())
        })
    }

    pub async fn remove(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM guardrail.products
                WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map_err(|err| {
            error!("Failed to remove product {id}: {err}");
            RepoError::DatabaseError("Failed to remove product".to_string())
        })?;

        Ok(())
    }

    pub async fn count(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<i64, RepoError> {
        sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM guardrail.products
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(|err| {
            error!("Failed to count products: {err}");
            RepoError::DatabaseError("Failed to count products".to_string())
        })
        .map(|count| count.unwrap_or(0))
    }
}
