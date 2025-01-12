use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use time::OffsetDateTime;
use tower_sessions::{
    session::{Id, Record},
    session_store, ExpiredDeletion, Session, SessionStore,
};

#[derive(Clone, Debug)]
pub struct SeaOrmSessionStore {
    db: DatabaseConnection,
}

impl SeaOrmSessionStore {
    pub fn new(db: DatabaseConnection) -> SeaOrmSessionStore {
        Self { db }
    }
}
#[async_trait]
impl ExpiredDeletion for SeaOrmSessionStore {
    async fn delete_expired(&self) -> session_store::Result<()> {
        let now = Utc::now().naive_utc();
        entities::entity::prelude::Session::delete_many()
            .filter(entities::entity::session::Column::ExpiresAt.lt(now))
            .exec(&self.db)
            .await
            .map_err(SeaStoreError::SeaError)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for SeaOrmSessionStore {
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let expiry_date = NaiveDateTime::from_timestamp_opt(
            record
                .expiry_date
                .to_offset(time::UtcOffset::UTC)
                .unix_timestamp(),
            0,
        );

        let data = entities::entity::session::ActiveModel {
            id: Set(record.id.to_string()),
            expires_at: Set(expiry_date),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            data: Set(rmp_serde::to_vec(&record).map_err(SeaStoreError::Encode)?),
        };
        entities::entity::prelude::Session::insert(data)
            .on_conflict(
                OnConflict::column(migration::SessionColumns::Id)
                    .update_columns([migration::SessionColumns::Data])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(SeaStoreError::SeaError)?;

        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let record = entities::entity::prelude::Session::find_by_id(session_id.to_string())
            .one(&self.db)
            .await
            .map_err(SeaStoreError::SeaError)?;

        if let Some(record) = record {
            let expires_at = record.expires_at.and_then(|t| {
                time::OffsetDateTime::from_unix_timestamp(t.and_utc().timestamp())
                    .ok()
                    .map(|x| x.to_offset(time::UtcOffset::UTC))
            });

            if let Some(expires_at) = expires_at {
                if expires_at > OffsetDateTime::now_utc() {
                    return Ok(Some(
                        rmp_serde::from_slice(&record.data).map_err(SeaStoreError::Decode)?,
                    ));
                }
            }
        }
        Ok(None)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        entities::entity::prelude::Session::delete_by_id(session_id.to_string())
            .exec(&self.db)
            .await
            .map_err(SeaStoreError::SeaError)?;
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SeaStoreError {
    #[error(transparent)]
    SeaError(#[from] sea_orm::error::DbErr),

    #[error(transparent)]
    Encode(#[from] rmp_serde::encode::Error),

    #[error(transparent)]
    Decode(#[from] rmp_serde::decode::Error),
}

impl From<SeaStoreError> for session_store::Error {
    fn from(err: SeaStoreError) -> Self {
        match err {
            SeaStoreError::SeaError(inner) => session_store::Error::Backend(inner.to_string()),
            SeaStoreError::Decode(inner) => session_store::Error::Decode(inner.to_string()),
            SeaStoreError::Encode(inner) => session_store::Error::Encode(inner.to_string()),
        }
    }
}

fn is_active(session: &Session) -> bool {
    let expiry_date = session.expiry_date();
    expiry_date > OffsetDateTime::now_utc()
}
