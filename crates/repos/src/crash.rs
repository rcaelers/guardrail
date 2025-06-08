use sqlx::{Postgres, QueryBuilder};

use crate::{
    Repo,
    error::{RepoError, handle_sql_error},
};
use common::QueryParams;
use data::crash::{Crash, NewCrash};
pub struct CrashRepo {}

impl CrashRepo {
    pub async fn get_by_id(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        id: uuid::Uuid,
    ) -> Result<Option<Crash>, RepoError> {
        sqlx::query_as!(
            Crash,
            r#"
                SELECT *
                FROM core.crashes
                WHERE core.crashes.id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn get_all(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        params: QueryParams,
    ) -> Result<Vec<Crash>, RepoError> {
        let mut builder = QueryBuilder::new("SELECT * from core.crashes");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "signature", "state", "created_at", "updated_at"],
            &["signature"],
        )?;

        let query = builder.build_query_as();

        query.fetch_all(executor).await.map_err(handle_sql_error)
    }

    pub async fn create(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        crash: NewCrash,
    ) -> Result<uuid::Uuid, RepoError> {
        sqlx::query_scalar!(
            r#"
                INSERT INTO core.crashes
                  (
                    id,
                    product_id,
                    minidump,
                    report,
                    signature
                  )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING
                  id
            "#,
            match crash.id {
                Some(id) => id,
                None => uuid::Uuid::new_v4(),
            },
            crash.product_id,
            crash.minidump,
            crash.report,
            crash.signature
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
    }

    pub async fn update(
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        crash: Crash,
    ) -> Result<Option<uuid::Uuid>, RepoError> {
        sqlx::query_scalar!(
            r#"
            UPDATE core.crashes
                SET minidump = $1, report = $2, signature = $3
                WHERE id = $4
                RETURNING id
            "#,
            crash.minidump,
            crash.report,
            crash.signature,
            crash.id,
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
                DELETE FROM core.crashes
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
                FROM core.crashes
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
