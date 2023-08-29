use std::sync::Arc;

use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::model::base::BaseRepo;
use crate::model::product::ProductRepo;
use crate::model::version::{VersionDto, VersionRepo};

pub struct Version;

#[derive(Debug, Deserialize)]
pub struct VersionRequestParams {
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
}

impl Version {
    pub async fn create(
        State(state): State<Arc<AppState>>,
        Json(payload): Json<VersionRequestParams>,
    ) -> impl IntoResponse {
        let product_id = ProductRepo::get_by_name(&state.db, &payload.product)
            .await
            .map(|product| product.id)
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
        let dto = VersionDto {
            product_id,
            name: payload.name,
            hash: payload.hash,
            tag: payload.tag,
        };
        VersionRepo::create(&state.db, dto)
            .await
            .map(|_| (StatusCode::OK, Json(serde_json::json!({ "result": "ok" }))))
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })
    }

    pub async fn update_by_id(
        Path(id): Path<uuid::Uuid>,
        State(state): State<Arc<AppState>>,
        Json(payload): Json<VersionRequestParams>,
    ) -> impl IntoResponse {
        let dto = VersionDto {
            product_id: id,
            name: payload.name,
            hash: payload.hash,
            tag: payload.tag,
        };
        VersionRepo::update(&state.db, id, dto)
            .await
            .map(|_| (StatusCode::OK, Json(serde_json::json!({ "result": "ok" }))))
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })
    }

    pub async fn query(State(state): State<Arc<AppState>>) -> impl IntoResponse {
        VersionRepo::get_all(&state.db)
            .await
            .map(|versions| (StatusCode::OK, serde_json::json!(versions).to_string()))
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })
    }

    pub async fn get_by_id(
        Path(id): Path<uuid::Uuid>,
        State(state): State<Arc<AppState>>,
    ) -> impl IntoResponse {
        VersionRepo::get_by_id(&state.db, id)
            .await
            .map(|version| (StatusCode::OK, serde_json::json!(version).to_string()))
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })
    }

    pub async fn remove_by_id(
        Path(id): Path<uuid::Uuid>,
        State(state): State<Arc<AppState>>,
    ) -> impl IntoResponse {
        VersionRepo::delete(&state.db, id).await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateVersionParams {
    pub name: String,
    pub id: Option<uuid::Uuid>,
    pub report_api_key: Option<String>,
    pub symbol_api_key: Option<String>,
}
