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
                FROM guardrail.crashes
                WHERE guardrail.crashes.id = $1
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
        let mut builder = QueryBuilder::new("SELECT * from guardrail.crashes");
        Repo::build_query(
            &mut builder,
            &params,
            &["id", "info", "state", "created_at", "updated_at"],
            &["info"],
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
                INSERT INTO guardrail.crashes
                  (
                    id,
                    product_id,
                    minidump,
                    info,
                    report,
                    version,
                    channel,
                    build_id,
                    commit
                  )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                RETURNING
                  id
            "#,
            match crash.id {
                Some(id) => id,
                None => uuid::Uuid::new_v4(),
            },
            crash.product_id,
            crash.minidump,
            crash.info,
            crash.report,
            crash.version,
            crash.channel,
            crash.build_id,
            crash.commit,
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
            UPDATE guardrail.crashes
                SET minidump = $1, info = $2, report = $3, version = $4, channel = $5, build_id = $6, commit = $7
                WHERE id = $8
                RETURNING id
            "#,
            crash.minidump,
            crash.info,
            crash.report,
            crash.version,
            crash.channel,
            crash.build_id,
            crash.commit,
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
                DELETE FROM guardrail.crashes
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
                FROM guardrail.crashes
            "#
        )
        .fetch_one(executor)
        .await
        .map_err(handle_sql_error)
        .map(|count| count.unwrap_or(0))
    }
}
