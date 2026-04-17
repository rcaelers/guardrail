use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
};
use common::QueryParams;
use data::crash::{Crash, NewCrash};

pub struct CrashRepo {}

impl CrashRepo {
    pub async fn get_by_id(db: &Surreal<Any>, id: uuid::Uuid) -> Result<Option<Crash>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('crashes', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_all(db: &Surreal<Any>, params: QueryParams) -> Result<Vec<Crash>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "signature", "state", "created_at", "updated_at"],
            &["signature"],
        )?;

        let query = format!("SELECT *, meta::id(id) as id FROM crashes{suffix}");
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(db: &Surreal<Any>, crash: NewCrash) -> Result<uuid::Uuid, RepoError> {
        let id = crash.id.unwrap_or_else(uuid::Uuid::new_v4);
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('crashes', $id) CONTENT {
                product_id: $product_id,
                minidump: $minidump,
                report: $report,
                signature: $signature,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.to_string()))
            .bind(("product_id", crash.product_id))
            .bind(("minidump", crash.minidump))
            .bind(("report", crash.report))
            .bind(("signature", crash.signature))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(db: &Surreal<Any>, crash: Crash) -> Result<Option<uuid::Uuid>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('crashes', $id) SET
                minidump = $minidump,
                report = $report,
                signature = $signature,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", crash.id.to_string()))
            .bind(("minidump", crash.minidump))
            .bind(("report", crash.report))
            .bind(("signature", crash.signature))
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows.first().and_then(|r| {
            r.get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
        }))
    }

    pub async fn remove(db: &Surreal<Any>, id: uuid::Uuid) -> Result<(), RepoError> {
        db.query("DELETE type::record('crashes', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(db: &Surreal<Any>) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM crashes GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }
}
