// Based on https://github.com/maxcountryman/tower-sessions-stores/blob/main/sqlx-store/src/postgres_store.rs
// Copyright (c) 2024 Max Countryman

use async_trait::async_trait;
use sqlx::{PgConnection, PgPool};
use tower_sessions::{
    ExpiredDeletion, SessionStore,
    session::{Id, Record},
    session_store,
};

#[derive(thiserror::Error, Debug)]
pub enum SqlxStoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Encode(#[from] rmp_serde::encode::Error),
    #[error(transparent)]
    Decode(#[from] rmp_serde::decode::Error),
}

impl From<SqlxStoreError> for session_store::Error {
    fn from(err: SqlxStoreError) -> Self {
        match err {
            SqlxStoreError::Sqlx(inner) => session_store::Error::Backend(inner.to_string()),
            SqlxStoreError::Decode(inner) => session_store::Error::Decode(inner.to_string()),
            SqlxStoreError::Encode(inner) => session_store::Error::Encode(inner.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn id_exists(&self, conn: &mut PgConnection, id: &Id) -> session_store::Result<bool> {
        Ok(sqlx::query_scalar(r#"select exists(select 1 from core.sessions where id = $1)"#)
            .bind(id.to_string())
            .fetch_one(conn)
            .await
            .map_err(SqlxStoreError::Sqlx)?)
    }

    async fn save_with_conn(
        &self,
        conn: &mut PgConnection,
        record: &Record,
    ) -> session_store::Result<()> {
        sqlx::query(
            r#"
            insert into core.sessions (id, data, expires_at)
            values ($1, $2, $3)
            on conflict (id) do update
            set
              data = excluded.data,
              expires_at = excluded.expires_at
            "#,
        )
        .bind(record.id.to_string())
        .bind(rmp_serde::to_vec(&record).map_err(SqlxStoreError::Encode)?)
        .bind(chrono::DateTime::from_timestamp(
            record.expiry_date.unix_timestamp(),
            record.expiry_date.nanosecond(),
        ))
        .execute(conn)
        .await
        .map_err(SqlxStoreError::Sqlx)?;

        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for PostgresStore {
    async fn delete_expired(&self) -> session_store::Result<()> {
        sqlx::query(
            r#"
            delete from core.sessions
            where expires_at < (now() at time zone 'utc')
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(SqlxStoreError::Sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for PostgresStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut tx = self.pool.begin().await.map_err(SqlxStoreError::Sqlx)?;

        while self.id_exists(&mut tx, &record.id).await? {
            record.id = Id::default();
        }
        self.save_with_conn(&mut tx, record).await?;
        tx.commit().await.map_err(SqlxStoreError::Sqlx)?;
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let mut conn = self.pool.acquire().await.map_err(SqlxStoreError::Sqlx)?;
        self.save_with_conn(&mut conn, record).await
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let record_value: Option<(Vec<u8>,)> = sqlx::query_as(
            r#"select data from core.sessions
               where id = $1 and expires_at > $2
               "#,
        )
        .bind(session_id.to_string())
        .bind(chrono::Utc::now().naive_utc())
        .fetch_optional(&self.pool)
        .await
        .map_err(SqlxStoreError::Sqlx)?;

        if let Some((data,)) = record_value {
            Ok(Some(rmp_serde::from_slice(&data).map_err(SqlxStoreError::Decode)?))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        sqlx::query(r#"delete from core.sessions where id = $1"#)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(SqlxStoreError::Sqlx)?;
        Ok(())
    }
}
