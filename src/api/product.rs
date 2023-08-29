use std::sync::Arc;

use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::model::base::BaseRepo;
use crate::model::product::{ProductDto, ProductRepo};
pub struct Product;

impl Product {
    pub async fn create(
        State(state): State<Arc<AppState>>,
        Json(payload): Json<ProductDto>,
    ) -> impl IntoResponse {
        ProductRepo::create(&state.db, payload)
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
        Json(payload): Json<ProductDto>,
    ) -> impl IntoResponse {
        ProductRepo::update(&state.db, id, payload)
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
        ProductRepo::get_all(&state.db)
            .await
            .map(|products| (StatusCode::OK, serde_json::json!(products).to_string()))
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
        ProductRepo::get_by_id(&state.db, id)
            .await
            .map(|product| (StatusCode::OK, serde_json::json!(product).to_string()))
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
        ProductRepo::delete(&state.db, id).await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProductParams {
    pub name: String,
    pub id: Option<uuid::Uuid>,
    pub report_api_key: Option<String>,
    pub symbol_api_key: Option<String>,
}
