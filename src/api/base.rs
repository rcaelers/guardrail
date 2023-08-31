use async_trait::async_trait;
use axum::http::{header, HeaderMap};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel};
use serde::de::DeserializeOwned;
use std::sync::Arc;

use axum::extract::{Json, Path, State};

use crate::app_state::AppState;
use crate::model::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};

use super::error::ApiError;

#[async_trait]
pub trait BaseApi<Repo: BaseRepo + Send>
where
    // TODO: this implementation detail should be hidden
    <<Repo::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
        IntoActiveModel<Repo::ActiveModel> + HasId,
{
    async fn req(
        _db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        Ok(json)
    }

    async fn resp(
        _db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        Ok(json)
    }

    fn json_content_type(headers: &HeaderMap) -> bool {
        let mime = headers
            .get(header::CONTENT_TYPE)
            .and_then(|content_type| content_type.to_str().ok())
            .and_then(|content_type| content_type.parse::<mime::Mime>().ok());

        mime.map(|mime| {
            mime.type_() == "application"
                && (mime.subtype() == "json" || mime.suffix().map_or(false, |name| name == "json"))
        })
        .unwrap_or(false)
    }

    async fn to_json<T>(s: String) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
    {
        let deserializer = &mut serde_json::Deserializer::from_str(s.as_str());
        let value = match serde_path_to_error::deserialize(deserializer) {
            Ok(value) => value,
            Err(err) => {
                let apierror = match err.inner().classify() {
                    serde_json::error::Category::Data => ApiError::Failure,
                    serde_json::error::Category::Syntax => ApiError::Failure,
                    serde_json::error::Category::Eof => ApiError::Failure,
                    serde_json::error::Category::Io => ApiError::Failure,
                };
                return Err(apierror);
            }
        };
        Ok(value)
    }

    async fn process_payload<T>(
        db: &DatabaseConnection,
        payload: String,
        headers: HeaderMap,
    ) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
    {
        if !Self::json_content_type(&headers) {
            return Err(ApiError::Failure);
        }
        let j = serde_json::from_str(payload.as_str())?;
        let j = Self::req(db, j).await?;
        Ok(Self::to_json::<T>(j.to_string()).await?)
    }

    async fn create(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap,
        payload: String,
    ) -> Result<String, ApiError>
    where
        <Repo as BaseRepo>::CreateDto: 'async_trait,
    {
        let p: <Repo as BaseRepo>::CreateDto =
            Self::process_payload(&state.db, payload, headers).await?;
        Repo::create(&state.db, p)
            .await
            .map(|id| (serde_json::json!({ "result": "ok", "id": id }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    async fn update_by_id(
        Path(id): Path<Repo::PrimaryKeyType>,
        State(state): State<Arc<AppState>>,
        Json(payload): Json<Repo::UpdateDto>,
    ) -> Result<String, ApiError>
    where
        <Repo as BaseRepo>::UpdateDto: 'async_trait,
        <Repo as BaseRepo>::PrimaryKeyType: 'async_trait,
    {
        Repo::update(&state.db, id, payload)
            .await
            .map(|_| (serde_json::json!({ "result": "ok"}).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    async fn query(State(state): State<Arc<AppState>>) -> Result<String, ApiError> {
        Repo::get_all(&state.db)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "payload": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    async fn get_by_id(
        Path(id): Path<Repo::PrimaryKeyType>,
        State(state): State<Arc<AppState>>,
    ) -> Result<String, ApiError>
    where
        <Repo as BaseRepo>::PrimaryKeyType: 'async_trait,
    {
        Repo::get_by_id(&state.db, id)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "payload": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    async fn remove_by_id(
        Path(id): Path<Repo::PrimaryKeyType>,
        State(state): State<Arc<AppState>>,
    ) -> Result<String, ApiError>
    where
        <Repo as BaseRepo>::PrimaryKeyType: 'async_trait,
    {
        Repo::delete(&state.db, id)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "id": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }
}

#[async_trait]
pub trait BaseApiWithSecondaryKey<Repo: BaseRepoWithSecondaryKey + Send>
where
    // TODO: this implementation detail should be hidden
    <<Repo::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
        IntoActiveModel<Repo::ActiveModel> + HasId,
{
    async fn get_by_id(
        Path(id): Path<Repo::SecondaryKeyType>,
        State(state): State<Arc<AppState>>,
    ) -> Result<String, ApiError>
    where
        <Repo as BaseRepo>::PrimaryKeyType: 'async_trait,
        sea_orm::Value: From<<Repo as BaseRepoWithSecondaryKey>::SecondaryKeyType> + 'async_trait,
        Repo::PrimaryKeyType:
            From<<Repo as BaseRepoWithSecondaryKey>::SecondaryKeyType> + 'async_trait,
        <Repo as BaseRepoWithSecondaryKey>::SecondaryKeyType: 'async_trait,
    {
        let mut r = Repo::get_by_id(&state.db, Repo::PrimaryKeyType::from(id.clone()))
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "id": p }).to_string()))
            .map_err(ApiError::DatabaseError);

        if r.is_err() {
            r = Repo::get_by_secondary_id(&state.db, id)
                .await
                .map(|p| (serde_json::json!({ "result": "ok", "id": p }).to_string()))
                .map_err(ApiError::DatabaseError);
        }
        r
    }
}
