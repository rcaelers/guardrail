use sqlx::{Postgres, QueryBuilder};

use crate::{
    Repo,
    error::{RepoError, handle_sql_error},
};
use common::QueryParams;
use data::symbols::{NewSymbols, Symbols};

pub struct SymbolsRepo {}

impl SymbolsRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<Symbols>, RepoError> {
        sqlx::query_as!(
            Symbols,
            r#"
                SELECT *
                FROM guardrail.symbols
                WHERE guardrail.symbols.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_by_module_and_build_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        build_id: &str,
        module_id: &str,
    ) -> Result<Option<Symbols>, RepoError> {
        sqlx::query_as!(
            Symbols,
            r#"
                SELECT *
                FROM guardrail.symbols
                WHERE guardrail.symbols.build_id = $1 AND guardrail.symbols.module_id = $2
            "#,
            build_id,
            module_id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<Symbols>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from guardrail.symbols");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "os", "arch", "build_id", "module_id", "file_location"],
            &["os", "arch", "build_id", "module_id"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        symbols: NewSymbols,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO guardrail.symbols
                  (
                    os,
                    arch,
                    build_id,
                    module_id,
                    file_location,
                    product_id,
                    version_id
                  )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING
                  id
            "#,
            symbols.os,
            symbols.arch,
            symbols.build_id,
            symbols.module_id,
            symbols.file_location,
            symbols.product_id,
            symbols.version_id
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        symbols: Symbols,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
                UPDATE guardrail.symbols
                SET os = $1, arch = $2, build_id = $3, module_id = $4, file_location = $5
                WHERE id = $6
                RETURNING id
            "#,
            symbols.os,
            symbols.arch,
            symbols.build_id,
            symbols.module_id,
            symbols.file_location,
            symbols.id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn remove(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<(), RepoError> {
        sqlx::query!(
            r#"
                DELETE FROM guardrail.symbols
                WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map_err(handle_sql_error)
        .map(|_| ())
    }

    pub async fn count(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
    ) -> Result<i64, RepoError> {
        sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM guardrail.symbols
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
