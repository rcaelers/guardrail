use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
    record_key,
};
use common::QueryParams;
use data::annotation::{Annotation, NewAnnotation};

pub struct AnnotationsRepo {}

impl AnnotationsRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<Annotation>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, meta::id(crash_id) as crash_id, \
                 meta::id(product_id) as product_id FROM ONLY type::record('annotations', $id)",
            )
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_all(
        db: &Surreal<Any>,
        params: QueryParams,
    ) -> Result<Vec<Annotation>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "key", "source", "value"],
            &["key", "source", "value"],
        )?;

        let query = format!(
            "SELECT *, meta::id(id) as id, meta::id(crash_id) as crash_id, \
             meta::id(product_id) as product_id FROM annotations{suffix}"
        );
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(db: &Surreal<Any>, annotation: NewAnnotation) -> Result<String, RepoError> {
        if !["submission", "user", "script"].contains(&annotation.source.as_str()) {
            return Err(RepoError::InvalidColumn(format!(
                "Invalid annotation source: {}",
                annotation.source
            )));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('annotations', $id) CONTENT {
                key: $key,
                source: $source,
                value: $value,
                crash_id: type::record('crashes', $crash_id),
                product_id: type::record('products', $product_id),
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.clone()))
            .bind(("key", annotation.key.clone()))
            .bind(("source", annotation.source.clone()))
            .bind(("value", annotation.value.clone()))
            .bind(("crash_id", record_key(&annotation.crash_id)))
            .bind(("product_id", record_key(&annotation.product_id)))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(
        db: &Surreal<Any>,
        annotation: Annotation,
    ) -> Result<Option<String>, RepoError> {
        if !["submission", "user", "script"].contains(&annotation.source.as_str()) {
            return Err(RepoError::InvalidColumn(format!(
                "Invalid annotation source: {}",
                annotation.source
            )));
        }

        let mut result = db
            .query(
                "UPDATE type::record('annotations', $id) SET
                key = $key,
                source = $source,
                value = $value,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", annotation.id.clone()))
            .bind(("key", annotation.key.clone()))
            .bind(("source", annotation.source.clone()))
            .bind(("value", annotation.value.clone()))
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
        db.query("DELETE type::record('annotations', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(db: &Surreal<Any>) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM annotations GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }

    pub async fn get_by_crash_id(
        db: &Surreal<Any>,
        crash_id: impl ToString,
        params: QueryParams,
    ) -> Result<Vec<Annotation>, RepoError> {
        let suffix = if !params.sorting.is_empty() || params.range.is_some() {
            let mut p = params.clone();
            p.filter = None;
            Repo::build_query_suffix(&p, &["id", "key", "source", "value", "created_at"], &[])?
        } else {
            String::new()
        };

        let query = format!(
            "SELECT *, meta::id(id) as id, meta::id(crash_id) as crash_id, \
             meta::id(product_id) as product_id FROM annotations \
             WHERE crash_id = type::record('crashes', $crash_id){suffix}"
        );
        let mut result = db
            .query(&query)
            .bind(("crash_id", record_key(crash_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }
}
