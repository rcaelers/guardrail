use async_trait::async_trait;
use chrono::NaiveDateTime;
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait, Set};
use tower_sessions::{
    session::{SessionError, SessionId},
    Session, SessionRecord, SessionStore,
};

use crate::entity;

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
impl SessionStore for SeaOrmSessionStore {
    type Error = SeaStoreError;

    async fn save(&self, session_record: &SessionRecord) -> Result<(), Self::Error> {
        let expires_at = session_record.expiration_time().and_then(|t| {
            NaiveDateTime::from_timestamp_opt(t.to_offset(time::UtcOffset::UTC).unix_timestamp(), 0)
        });

        let data = entity::session::ActiveModel {
            id: Set(session_record.id().to_string()),
            expires_at: Set(expires_at),
            data: Set(rmp_serde::to_vec(&session_record.data())?),
        };
        entity::session::Entity::insert(data)
            .on_conflict(
                OnConflict::column(migration::SessionColumns::Id)
                    .update_columns([migration::SessionColumns::Data])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn load(&self, session_id: &SessionId) -> Result<Option<Session>, Self::Error> {
        let record = crate::entity::prelude::Session::find_by_id(session_id.to_string())
            .one(&self.db)
            .await?;

        if let Some(record) = record {
            let session_id = SessionId::try_from(record.id)?;
            let expires_at = record.expires_at.and_then(|t| {
                time::OffsetDateTime::from_unix_timestamp(t.timestamp())
                    .ok()
                    .map(|x| x.to_offset(time::UtcOffset::UTC))
            });

            let session_record =
                SessionRecord::new(session_id, expires_at, rmp_serde::from_slice(&record.data)?);
            Ok(Some(session_record.into()))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &SessionId) -> Result<(), Self::Error> {
        crate::entity::prelude::Session::delete_by_id(session_id.to_string())
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SeaStoreError {
    #[error("Session error: {0}")]
    SessionError(#[from] SessionError),

    #[error("SeaORM error: {0}")]
    SeaError(#[from] sea_orm::error::DbErr),

    #[error("Rust MsgPack encode error: {0}")]
    SerdeMsgPackEncode(#[from] rmp_serde::encode::Error),

    #[error("Rust MsgPack decode error: {0}")]
    SerdeMsgPackDecode(#[from] rmp_serde::decode::Error),
}
