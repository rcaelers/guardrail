use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use time::OffsetDateTime;
use tower_sessions::{session::Id, ExpiredDeletion, Session, SessionStore};

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
impl ExpiredDeletion for SeaOrmSessionStore {
    async fn delete_expired(&self) -> Result<(), Self::Error> {
        let now = Utc::now().naive_utc();
        entity::session::Entity::delete_many()
            .filter(entity::session::Column::ExpiresAt.lt(now))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for SeaOrmSessionStore {
    type Error = SeaStoreError;

    async fn save(&self, session: &Session) -> Result<(), Self::Error> {
        let expiry_date = NaiveDateTime::from_timestamp_opt(
            session
                .expiry_date()
                .to_offset(time::UtcOffset::UTC)
                .unix_timestamp(),
            0,
        );

        let data = entity::session::ActiveModel {
            id: Set(session.id().to_string()),
            expires_at: Set(expiry_date),
            data: Set(rmp_serde::to_vec(&session)?),
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

    async fn load(&self, session_id: &Id) -> Result<Option<Session>, Self::Error> {
        let now = Utc::now().naive_utc();
        let record = crate::entity::prelude::Session::find_by_id(session_id.to_string())
            .one(&self.db)
            .await?;

        if let Some(record) = record {
            let session_id = Id::try_from(record.id);
            let expires_at = record.expires_at.and_then(|t| {
                time::OffsetDateTime::from_unix_timestamp(t.timestamp())
                    .ok()
                    .map(|x| x.to_offset(time::UtcOffset::UTC))
            });

            if let Some(expires_at) = expires_at {
                if expires_at > OffsetDateTime::now_utc() {
                    Ok(Some(rmp_serde::from_slice(&record.data)?))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> Result<(), Self::Error> {
        crate::entity::prelude::Session::delete_by_id(session_id.to_string())
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SeaStoreError {
    //#[error("Session error: {0}")]
    //SessionError(#[from] SessionError),
    #[error("SeaORM error: {0}")]
    SeaError(#[from] sea_orm::error::DbErr),

    #[error("Rust MsgPack encode error: {0}")]
    SerdeMsgPackEncode(#[from] rmp_serde::encode::Error),

    #[error("Rust MsgPack decode error: {0}")]
    SerdeMsgPackDecode(#[from] rmp_serde::decode::Error),
}

fn is_active(session: &Session) -> bool {
    let expiry_date = session.expiry_date();
    expiry_date > OffsetDateTime::now_utc()
}
