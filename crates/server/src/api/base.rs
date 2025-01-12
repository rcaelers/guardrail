use async_trait::async_trait;
use axum::extract::{Json, Path, State};
use axum::http::{header, HeaderMap};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    ModelTrait,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    app_state::AppState,
    model::base::{HasId, Repo},
};

use super::error::ApiError;

pub struct Api;

#[async_trait]
pub trait ResourceFilter {
    async fn req(
        db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError>;
}

pub trait Resource {
    type Entity: EntityTrait<Model = Self::Data> + Send;
    type ActiveModel: ActiveModelTrait<Entity = Self::Entity> + ActiveModelBehavior + Send;

    type Data: ModelTrait<Entity = Self::Entity>
        + IntoActiveModel<Self::ActiveModel>
        + Clone
        + Send
        + Serialize
        + DeserializeOwned
        + HasId;

    type CreateData: IntoActiveModel<Self::ActiveModel>
        + Clone
        + Send
        + Serialize
        + DeserializeOwned;

    type UpdateData: IntoActiveModel<Self::ActiveModel>
        + Clone
        + Send
        + Serialize
        + DeserializeOwned;

    type Filter: ResourceFilter;
}

pub struct NoneFilter;

#[async_trait]
impl ResourceFilter for NoneFilter {
    async fn req(
        _db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        Ok(json)
    }
}

impl Api {
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

    async fn req<R>(
        db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError>
    where
        R: Resource,
    {
        <R::Filter as ResourceFilter>::req(db, json).await
    }

    async fn process_payload<R, T>(
        db: &DatabaseConnection,
        payload: String,
        headers: HeaderMap,
    ) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
        R: Resource,
    {
        if !Self::json_content_type(&headers) {
            return Err(ApiError::APIFailure(
                "Content-Type must be application/json".to_owned(),
            ));
        }
        let j = serde_json::from_str(payload.as_str())?;
        let j = Self::req::<R>(db, j).await?;
        Self::to_json::<T>(j.to_string()).await
    }

    pub async fn create<R>(
        State(state): State<AppState>,
        headers: HeaderMap,
        payload: String,
    ) -> Result<String, ApiError>
    where
        R: Resource,
    {
        let p: R::CreateData = Self::process_payload::<R, _>(&state.db, payload, headers).await?;
        Repo::create(&state.db, p)
            .await
            .map(|p|  (serde_json::json!({ "result": "ok", "id": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    pub async fn update<R>(
        Path(_id): Path<uuid::Uuid>,
        State(state): State<AppState>,
        Json(payload): Json<R::UpdateData>,
    ) -> Result<String, ApiError>
    where
        R: Resource,
    {
        Repo::update(&state.db, payload)
            .await
            .map(|_| (serde_json::json!({ "result": "ok"}).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    pub async fn get_all<R>(State(state): State<AppState>) -> Result<String, ApiError>
    where
        R: Resource,
    {
        Repo::get_all::<R::Entity>(&state.db)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "payload": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    pub async fn get_by_id<R>(
        Path(id): Path<uuid::Uuid>,
        State(state): State<AppState>,
    ) -> Result<String, ApiError>
    where
        R: Resource,
        <<R::Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType:
            From<uuid::Uuid>,
    {
        Repo::get_by_id::<R::Entity>(&state.db, id)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "payload": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }

    pub async fn remove_by_id<R>(
        Path(id): Path<uuid::Uuid>,
        State(state): State<AppState>,
    ) -> Result<String, ApiError>
    where
        R: Resource,
        <<R::Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType:
            From<uuid::Uuid>,
    {
        Repo::delete_by_id::<R::Entity>(&state.db, id)
            .await
            .map(|p| (serde_json::json!({ "result": "ok", "id": p }).to_string()))
            .map_err(ApiError::DatabaseError)
    }
}

#[cfg(test)]
pub mod tests {
    use axum::extract::DefaultBodyLimit;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};
    use std::{io::IsTerminal, sync::Arc};
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;
    use url::Url;
    use webauthn_rs::WebauthnBuilder;

    use crate::api::routes::routes_test;
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

        let rp_id = "localhost";
        let rp_origin = Url::parse("http://localhost:8080").expect("Invalid URL");
        let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");

        let builder = builder.rp_name("Guardrail");

        // let auth_client = Arc::new(crate::auth::oidc::test_stubs::OidcClientStub {});
        let state = AppState {
            db,
            leptos_options: Default::default(),
            routes: vec![],
            // auth_client,
            webauthn: Arc::new(builder.build().expect("Invalid configuration")),
        };

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
