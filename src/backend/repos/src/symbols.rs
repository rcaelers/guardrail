use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    Repo,
    error::{RepoError, handle_surreal_error},
};
use common::QueryParams;
use data::symbols::{NewSymbols, Symbols};

pub struct SymbolsRepo {}

impl SymbolsRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: uuid::Uuid,
    ) -> Result<Option<Symbols>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM ONLY type::record('symbols', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_module_and_build_id(
        db: &Surreal<Any>,
        build_id: &str,
        module_id: &str,
    ) -> Result<Option<Symbols>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id FROM symbols WHERE build_id = $build_id AND module_id = $module_id LIMIT 1")
            .bind(("build_id", build_id.to_owned()))
            .bind(("module_id", module_id.to_owned()))
            .await
            .map_err(handle_surreal_error)?;
        let symbols: Vec<Symbols> = crate::take_many(&mut result, 0)?;
        Ok(symbols.into_iter().next())
    }

    pub async fn get_all(
        db: &Surreal<Any>,
        params: QueryParams,
    ) -> Result<Vec<Symbols>, RepoError> {
        let suffix = Repo::build_query_suffix(
            &params,
            &["id", "os", "arch", "build_id", "module_id", "storage_path"],
            &["os", "arch", "build_id", "module_id"],
        )?;

        let query = format!("SELECT *, meta::id(id) as id FROM symbols{suffix}");
        let mut builder = db.query(&query);

        if let Some(filter) = params.filter {
            builder = builder.bind(("filter", filter));
        }

        let mut result = builder.await.map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(db: &Surreal<Any>, symbols: NewSymbols) -> Result<uuid::Uuid, RepoError> {
        let id = uuid::Uuid::new_v4();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('symbols', $id) CONTENT {
                os: $os,
                arch: $arch,
                build_id: $build_id,
                module_id: $module_id,
                storage_path: $storage_path,
                product_id: $product_id,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.to_string()))
            .bind(("os", symbols.os.clone()))
            .bind(("arch", symbols.arch.clone()))
            .bind(("build_id", symbols.build_id.clone()))
            .bind(("module_id", symbols.module_id.clone()))
            .bind(("storage_path", symbols.storage_path.clone()))
            .bind(("product_id", symbols.product_id))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(
        db: &Surreal<Any>,
        symbols: Symbols,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        let mut result = db
            .query(
                "UPDATE type::record('symbols', $id) SET
                os = $os,
                arch = $arch,
                build_id = $build_id,
                module_id = $module_id,
                storage_path = $storage_path,
                updated_at = time::now()
            RETURN meta::id(id) as id",
            )
            .bind(("id", symbols.id.to_string()))
            .bind(("os", symbols.os.clone()))
            .bind(("arch", symbols.arch.clone()))
            .bind(("build_id", symbols.build_id.clone()))
            .bind(("module_id", symbols.module_id.clone()))
            .bind(("storage_path", symbols.storage_path.clone()))
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
        db.query("DELETE type::record('symbols', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn count(db: &Surreal<Any>) -> Result<i64, RepoError> {
        let mut result = db
            .query("SELECT count() as count FROM symbols GROUP ALL")
            .await
            .map_err(handle_surreal_error)?;
        let rows: Vec<serde_json::Value> = result.take(0).map_err(handle_surreal_error)?;
        Ok(rows
            .first()
            .and_then(|r| r.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0))
    }
}
