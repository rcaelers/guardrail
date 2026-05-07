use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    error::{RepoError, handle_surreal_error},
    record_key,
};
use data::crash_group::{CrashGroup, NewCrashGroup};

pub struct CrashGroupRepo {}

impl CrashGroupRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<CrashGroup>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, meta::id(product_id) as product_id \
                 FROM ONLY type::record('crash_groups', $id)",
            )
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn find_by_fingerprint(
        db: &Surreal<Any>,
        product_id: &str,
        fingerprint: &str,
    ) -> Result<Option<CrashGroup>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, meta::id(product_id) as product_id \
                 FROM crash_groups \
                 WHERE product_id = type::record('products', $product_id) \
                   AND fingerprint = $fingerprint \
                 LIMIT 1",
            )
            .bind(("product_id", record_key(product_id)))
            .bind(("fingerprint", fingerprint.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    /// Create a new crash group. Returns the generated id.
    pub async fn create(db: &Surreal<Any>, group: NewCrashGroup) -> Result<String, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('crash_groups', $id) CONTENT {
                    product_id: type::record('products', $product_id),
                    fingerprint: $fingerprint,
                    signal: $signal,
                    count: 1,
                    first_seen: time::now(),
                    last_seen: time::now(),
                    status: 'new',
                    created_at: time::now(),
                    updated_at: time::now(),
                }",
            )
            .bind(("id", id.clone()))
            .bind(("product_id", record_key(&group.product_id)))
            .bind(("fingerprint", group.fingerprint))
            .bind(("signal", group.signal))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    /// Increment the crash count and push `last_seen` forward.
    /// Called for every crash that joins an existing group.
    pub async fn touch(db: &Surreal<Any>, id: &str) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('crash_groups', $id) SET \
                count += 1, \
                last_seen = time::now(), \
                updated_at = time::now()",
        )
        .bind(("id", record_key(id)))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }
}
