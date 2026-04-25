pub mod annotation;
pub mod api_token;
pub mod attachment;
pub mod crash;
pub mod crash_group;
pub mod credentials;
pub mod error;
pub mod product;
pub mod symbols;
pub mod user;

use serde::de::DeserializeOwned;
use surrealdb::IndexedResults;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::error;

use crate::error::{RepoError, handle_surreal_error};
use common::QueryParams;

#[derive(Debug, Clone)]
pub struct Repo {
    pub db: Surreal<Any>,
}

/// Extract a single optional result from a SurrealDB query response.
/// Uses serde_json::Value as intermediate to avoid requiring SurrealValue on data types.
pub fn take_one<T: DeserializeOwned>(
    result: &mut IndexedResults,
    index: usize,
) -> Result<Option<T>, RepoError> {
    let val: Option<serde_json::Value> = result.take(index).map_err(handle_surreal_error)?;
    match val {
        Some(v) => Ok(Some(
            serde_json::from_value(v).map_err(|e| RepoError::DatabaseError(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

/// Extract a vec of results from a SurrealDB query response.
pub fn take_many<T: DeserializeOwned>(
    result: &mut IndexedResults,
    index: usize,
) -> Result<Vec<T>, RepoError> {
    let vals: Vec<serde_json::Value> = result.take(index).map_err(handle_surreal_error)?;
    vals.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| RepoError::DatabaseError(e.to_string())))
        .collect()
}

pub fn record_key(id: impl AsRef<str>) -> String {
    let id = id.as_ref();
    id.split_once(':')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| id.to_string())
}

impl Repo {
    pub fn new(db: Surreal<Any>) -> Repo {
        Repo { db }
    }

    /// Create a user-scoped database handle by authenticating with a JWT.
    ///
    /// The returned `Surreal<Any>` has its own session (SurrealDB clones create
    /// independent sessions).  After `authenticate`, the session is treated as a
    /// *record user* and table permissions are enforced via the `$auth` variable
    /// populated from the `id` claim in the JWT.
    pub async fn authenticated(&self, jwt: &str) -> Result<Surreal<Any>, RepoError> {
        use surrealdb::opt::auth::Token;

        let db = self.db.clone(); // new session id
        let token = Token::from(jwt);
        db.authenticate(token).await.map_err(handle_surreal_error)?;
        Ok(db)
    }

    pub fn build_query_suffix(
        params: &QueryParams,
        allowed_columns: &[&str],
        filter_columns: &[&str],
    ) -> Result<String, RepoError> {
        let mut suffix = String::new();

        if let Some(_filter) = &params.filter {
            if filter_columns.is_empty() {
                error!("No filter columns specified but filter was provided");
                return Err(RepoError::InvalidColumn("No filter columns specified".to_string()));
            }

            suffix.push_str(" WHERE ");
            let conditions: Vec<String> = filter_columns
                .iter()
                .map(|col| {
                    if !allowed_columns.contains(col) {
                        return Err(RepoError::InvalidColumn(col.to_string()));
                    }
                    Ok(format!("string::lowercase({col}) CONTAINS string::lowercase($filter)"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            suffix.push_str(&conditions.join(" OR "));
        }

        if !params.sorting.is_empty() {
            suffix.push_str(" ORDER BY ");
            let orders: Vec<String> = params
                .sorting
                .iter()
                .map(|(col, order)| {
                    if !allowed_columns.contains(&col.as_str()) {
                        error!("Invalid column specified for sorting: {col}");
                        return Err(RepoError::InvalidColumn(col.clone()));
                    }
                    Ok(format!("{col} {}", order.to_sql()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            suffix.push_str(&orders.join(", "));
        }

        if let Some(range) = &params.range {
            suffix.push_str(&format!(" LIMIT {} START {}", range.len(), range.start));
        }

        Ok(suffix)
    }
}
