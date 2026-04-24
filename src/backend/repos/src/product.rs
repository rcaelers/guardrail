use std::collections::HashSet;

use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
    record_key,
};
use common::QueryParams;
use data::product::{NewProduct, Product};

pub struct ProductRepo {}

impl ProductRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<Product>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('products', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_name(db: &Surreal<Any>, name: &str) -> Result<Option<Product>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM products WHERE name = $name LIMIT 1")
            .bind(("name", name.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let products: Vec<Product> = crate::take_many(&mut result, 0)?;
        Ok(products.into_iter().next())
    }

    pub async fn get_all_names(db: &Surreal<Any>) -> Result<HashSet<String>, RepoError> {
        let mut result = db
            .query("SELECT name FROM products")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect())
    }

    pub async fn get_all(
        db: &Surreal<Any>,
        params: QueryParams,
    ) -> Result<Vec<Product>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &[
                "id",
                "name",
                "description",
                "public",
                "accepting_crashes",
                "metadata",
                "created_at",
                "updated_at",
            ],
            &["name", "description"],
        )?;

        let query = format!("SELECT *, meta::id(id) as id FROM products{suffix}");
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(db: &Surreal<Any>, product: NewProduct) -> Result<String, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let slug = product.name.to_lowercase().replace(' ', "-");
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('products', $id) CONTENT {
                name: $name,
                slug: $slug,
                description: $description,
                public: $public,
                accepting_crashes: true,
                metadata: $metadata,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.clone()))
            .bind(("name", product.name.clone()))
            .bind(("slug", slug))
            .bind(("description", product.description.clone()))
            .bind(("public", product.public))
            .bind(("metadata", product.metadata.clone()))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(db: &Surreal<Any>, product: Product) -> Result<Option<String>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('products', $id) SET
                name = $name,
                description = $description,
                public = $public,
                accepting_crashes = $accepting_crashes,
                metadata = $metadata,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", product.id.clone()))
            .bind(("name", product.name.clone()))
            .bind(("description", product.description.clone()))
            .bind(("public", product.public))
            .bind(("accepting_crashes", product.accepting_crashes))
            .bind(("metadata", product.metadata.clone()))
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
        db.query("DELETE type::record('products', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(db: &Surreal<Any>) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM products GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }
}
