pub mod annotation;
pub mod api_token;
pub mod attachment;
pub mod crash;
pub mod credentials;
pub mod error;
pub mod product;
pub mod symbols;
pub mod user;
pub mod version;

use sqlx::{Executor, pool::PoolConnection};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use tracing::error;

use crate::error::RepoError;
use common::QueryParams;

const ADMIN: &str = "admin";

#[derive(Debug, Clone)]
pub struct Repo {
    pub pool: PgPool,
}

impl Repo {
    pub fn new(pool: PgPool) -> Repo {
        Repo { pool }
    }

    async fn set_config(
        &self,
        conn: impl Executor<'_, Database = Postgres>,
        auth: &str,
    ) -> Result<(), RepoError> {
        sqlx::query("SELECT set_config('request.jwt.claims', json_build_object('username', $1::text)::text, false)")
                .bind(auth)
                .execute(conn)
                .await
                .map_err(|err| {
                    error!("Failed to set user to {}: {}", auth, err);
                    RepoError::TransactionError()
                }).
                map(|_| ())
    }

    pub async fn begin_admin(&self) -> Result<Transaction<'static, Postgres>, RepoError> {
        let mut transaction = self.pool.begin().await.map_err(|err| {
            error!("Failed to begin admin transaction: {}", err);
            RepoError::TransactionError()
        })?;
        self.set_config(&mut *transaction, ADMIN).await?;
        Ok(transaction)
    }

    pub async fn acquire_admin(&self) -> Result<PoolConnection<Postgres>, RepoError> {
        let mut con = self.pool.acquire().await.map_err(|err| {
            error!("Failed to acquire admin connection: {}", err);
            RepoError::TransactionError()
        })?;
        self.set_config(&mut *con, ADMIN).await?;
        Ok(con)
    }

    pub async fn acquire(&self, auth: &str) -> Result<PoolConnection<Postgres>, RepoError> {
        let mut con = self.pool.acquire().await.map_err(|err| {
            error!("Failed to acquire connection for {}: {}", auth, err);
            RepoError::TransactionError()
        })?;
        self.set_config(&mut *con, auth).await?;
        Ok(con)
    }

    pub async fn begin(self, auth: &str) -> Result<Transaction<'static, Postgres>, RepoError> {
        let mut transaction = self.pool.begin().await.map_err(|err| {
            error!("Failed to begin transaction for {}: {}", auth, err);
            RepoError::TransactionError()
        })?;
        self.set_config(&mut *transaction, auth).await?;
        Ok(transaction)
    }

    pub async fn end(&self, transaction: Transaction<'static, Postgres>) -> Result<(), RepoError> {
        transaction.commit().await.map_err(|err| {
            error!("Failed to commit transaction: {}", err);
            RepoError::TransactionError()
        })
    }

    pub fn build_query(
        builder: &mut QueryBuilder<Postgres>,
        params: &QueryParams,
        allowed_columns: &[&str],
        filter_columns: &[&str],
    ) -> Result<(), RepoError> {
        if let Some(filter) = &params.filter {
            if filter_columns.is_empty() {
                error!("No filter columns specified but filter was provided");
                return Err(RepoError::InvalidColumn("No filter columns specified".to_string()));
            }

            builder.push(" WHERE ");
            let mut separated = builder.separated(" OR ");
            for &col in filter_columns {
                if !allowed_columns.contains(&col) {
                    error!("Invalid column specified for filtering: {col}");
                    return Err(RepoError::InvalidColumn(col.to_string()));
                }
                separated.push(col);
                separated.push_unseparated(" ILIKE ");
                separated.push_bind_unseparated(format!("%{filter}%"));
            }
        }

        if !params.sorting.is_empty() {
            builder.push(" ORDER BY ");
            let mut separated = builder.separated(", ");

            for (col, col_sort) in &params.sorting {
                if !allowed_columns.contains(&col.as_str()) {
                    error!("Invalid column specified for sorting: {col}");
                    return Err(RepoError::InvalidColumn(col.clone()));
                }

                separated.push_unseparated(col);
                separated.push_unseparated(" ");
                separated.push_unseparated(col_sort.to_sql());
            }
        }

        if let Some(range) = &params.range {
            builder.push(" LIMIT ");
            builder.push_bind(range.len() as i64);
            builder.push(" OFFSET ");
            builder.push_bind(range.start as i64);
        }

        Ok(())
    }
}
