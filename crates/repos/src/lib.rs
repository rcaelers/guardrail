#![feature(cfg_match)]

pub mod attachment;
pub mod crash;
pub mod credentials;
pub mod error;
pub mod product;
pub mod symbols;
pub mod user;
pub mod version;

use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, ops::Range};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(String, SortOrder)>,
    pub range: Option<Range<usize>>,
    pub filter: Option<String>,
}

#[cfg(feature = "ssr")]
pub mod ssr {

    use sqlx::{Executor, pool::PoolConnection};
    use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};

    use crate::QueryParams;
    use crate::error::RepoError;

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
        ) -> Result<(), sqlx::Error> {
            sqlx::query("SELECT set_config('request.jwt.claims.email',$1::text,false)")
                .bind(auth)
                .execute(conn)
                .await?;

            Ok(())
        }

        pub async fn begin_admin(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
            let mut transaction = self.pool.begin().await?;
            self.set_config(&mut *transaction, ADMIN).await?;
            Ok(transaction)
        }

        pub async fn acquire_admin(&self) -> Result<PoolConnection<Postgres>, sqlx::Error> {
            let mut con = self.pool.acquire().await?;
            self.set_config(&mut *con, ADMIN).await?;
            Ok(con)
        }

        pub async fn acquire(&self, auth: &str) -> Result<PoolConnection<Postgres>, sqlx::Error> {
            let mut con = self.pool.acquire().await?;
            self.set_config(&mut *con, auth).await?;
            Ok(con)
        }

        pub async fn begin(
            self,
            auth: &str,
        ) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
            let mut transaction = self.pool.begin().await?;
            self.set_config(&mut *transaction, auth).await?;
            Ok(transaction)
        }

        pub fn build_query(
            builder: &mut QueryBuilder<Postgres>,
            params: &QueryParams,
            allowed_columns: &[&str],
            filter_columns: &[&str],
        ) -> Result<(), RepoError> {
            if !params.sorting.is_empty() {
                builder.push(" ORDER BY ");
                let mut separated = builder.separated(", ");

                for (col, col_sort) in &params.sorting {
                    if !allowed_columns.contains(&col.as_str()) {
                        return Err(RepoError::InvalidColumn(col.clone()));
                    }

                    separated.push_unseparated(col);
                    separated.push_unseparated(" ");
                    separated.push_unseparated(col_sort.to_sql());
                }
            }

            if let Some(range) = &params.range {
                builder.push(" OFFSET ");
                builder.push_bind(range.start as i64);
                builder.push(" LIMIT ");
                builder.push_bind(range.len() as i64);
            }

            if let Some(filter) = &params.filter {
                if filter_columns.is_empty() {
                    return Err(RepoError::InvalidColumn(
                        "No filter columns specified".to_string(),
                    ));
                }

                builder.push(" WHERE ");
                let mut separated = builder.separated(" OR ");
                for &col in filter_columns {
                    if !allowed_columns.contains(&col) {
                        return Err(RepoError::InvalidColumn(col.to_string()));
                    }
                    separated.push_unseparated(col);
                    separated.push_unseparated(" ILIKE ");
                    separated.push_bind(format!("%{}%", filter));
                }
            }

            Ok(())
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;

//}}
