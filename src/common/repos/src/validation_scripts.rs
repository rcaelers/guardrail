use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    error::{RepoError, handle_surreal_error},
    record_key,
};
use data::validation_script::ValidationScript;

pub struct ValidationScriptsRepo;

impl ValidationScriptsRepo {
    pub async fn list(
        db: &Surreal<Any>,
        product_id: &str,
    ) -> Result<Vec<ValidationScript>, RepoError> {
        let key = record_key(product_id);
        let mut result = db
            .query(
                "SELECT *, meta::id(id) AS id, meta::id(product_id) AS product_id \
                 FROM validation_scripts \
                 WHERE product_id = type::record('products', $id) \
                 ORDER BY created_at ASC",
            )
            .bind(("id", key))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(
        db: &Surreal<Any>,
        product_id: &str,
        name: &str,
        content: &str,
    ) -> Result<ValidationScript, RepoError> {
        let key = record_key(product_id);
        let name = name.to_string();
        let content = content.to_string();
        let mut result = db
            .query(
                "CREATE validation_scripts SET \
                 product_id = type::record('products', $id), \
                 name = $name, \
                 content = $content, \
                 created_at = time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id, meta::id(product_id) AS product_id",
            )
            .bind(("id", key))
            .bind(("name", name))
            .bind(("content", content))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<ValidationScript>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("create returned no row".into()))
    }

    pub async fn get(
        db: &Surreal<Any>,
        script_id: &str,
        product_id: &str,
    ) -> Result<Option<ValidationScript>, RepoError> {
        let sid = record_key(script_id);
        let pid = record_key(product_id);
        let mut result = db
            .query(
                "SELECT *, meta::id(id) AS id, meta::id(product_id) AS product_id \
                 FROM ONLY type::record('validation_scripts', $sid) \
                 WHERE product_id = type::record('products', $pid)",
            )
            .bind(("sid", sid))
            .bind(("pid", pid))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn delete(
        db: &Surreal<Any>,
        script_id: &str,
        product_id: &str,
    ) -> Result<(), RepoError> {
        let pid = record_key(product_id);
        let sid = record_key(script_id);
        db.query(
            "DELETE type::record('validation_scripts', $sid) \
             WHERE product_id = type::record('products', $pid)",
        )
        .bind(("sid", sid))
        .bind(("pid", pid))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }
}
