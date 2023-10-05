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
        let value = match serde_json::from_str::<T>(s.as_str()) {
            Ok(value) => value,
            Err(err) => return Err(ApiError::JsonError(err)),
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
            return Err(ApiError::APIFailure(
                "Content-Type must be application/json".to_owned(),
            ));
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

#[cfg(test)]
pub mod tests {
    use axum::extract::DefaultBodyLimit;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection, EntityTrait, IntoActiveModel};
    use std::{io::IsTerminal, sync::Arc};
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::api::routes::routes_test;
    use crate::model::base::{BaseRepo, HasId};
    use ::axum::Router;
    use ::axum_test::TestServer;

    use crate::app_state::AppState;

    pub async fn init_logging() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .with_ansi(std::io::stdout().is_terminal())
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    }

    pub async fn run_server() -> TestServer {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let auth_client = Arc::new(crate::auth::oidc::test_stubs::OidcClientStub {});
        let state = Arc::new(AppState { db, auth_client });

        let app = Router::new()
            // FIXME: duplicate code
            .nest("/api", routes_test().await)
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
            .with_state(state)
            .into_make_service();

        TestServer::new(app).unwrap()
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponse {
        pub result: String,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseFailed {
        pub result: String,
        pub error: String,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseWithId {
        pub result: String,
        pub id: String,
    }
}
