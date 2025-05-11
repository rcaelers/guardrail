use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use axum_extra::extract::WithRejection;
use data::api_token::ApiToken;
use object_store::PutPayload;
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use tracing::error;

use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, get_product_by_id, stream_to_s3, validate_api_token_for_product};

pub struct MinidumpApi;

#[derive(Debug, Deserialize)]
pub struct MinidumpRequestParams {
    // pub product: String,
    // pub version: String,
}

impl MinidumpRequestParams {
    pub fn validate(&self) -> Result<(), ApiError> {
        // if self.product.trim().is_empty() {
        //     return Err(ApiError::Failure("product name cannot be empty".to_string()));
        // }
        // if self.version.trim().is_empty() {
        //     return Err(ApiError::Failure("version cannot be empty".to_string()));
        // }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
    pub crash_id: Option<uuid::Uuid>,
}

impl MinidumpApi {
    const REQUIRED_FIELDS: &'static [&'static str] =
        &["product", "version", "channel", "commit", "buildid"];

    fn validate_content_type(
        content_type: &str,
        content_type_category: &str,
    ) -> Result<(), ApiError> {
        let is_valid = match content_type_category {
            "minidump" => {
                matches!(content_type, "application/octet-stream")
            }
            "attachment" => {
                !content_type.contains("text/html")
                    && !content_type.contains("application/javascript")
            }
            "annotation" => {
                content_type == "text/plain"
                    || content_type == "text/markdown"
                    || content_type.is_empty()
            }
            _ => false,
        };

        if !is_valid {
            error!("Invalid {} content type: {}", content_type_category, content_type);
            return Err(ApiError::Failure(format!(
                "invalid {content_type_category} content type: {content_type}"
            )));
        }
        Ok(())
    }

    fn validate_key(key: &str) -> Result<(), ApiError> {
        if !key.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            error!("Key contains non-printable ASCII characters: {}", key);
            return Err(ApiError::Failure(
                "key must contain only printable ASCII characters".to_string(),
            ));
        }
        Ok(())
    }

    fn extract_filename(field: &Field<'_>) -> Result<String, ApiError> {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        Ok(filename)
    }

    async fn handle_minidump_upload(
        field: Field<'_>,
        crash_info: &mut serde_json::Value,
        state: AppState,
    ) -> Result<(), ApiError> {
        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_content_type(&content_type, "minidump")?;

        let filename = Self::extract_filename(&field)?;
        let storage_filename = uuid::Uuid::new_v4();
        let storage_path = format!("minidumps/{storage_filename}");

        let file_size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!("Failed to stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        crash_info["minidump"] = serde_json::json!({
            "filename": filename,
            "mimetype": content_type,
            "size": file_size,
            "storage_path": storage_path,
            "storage_filename": storage_filename,
        });

        Ok(())
    }

    async fn handle_attachment_upload(
        field: Field<'_>,
        crash_info: &mut serde_json::Value,
        state: AppState,
    ) -> Result<(), ApiError> {
        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_content_type(&content_type, "attachment")?;

        let storage_filename = uuid::Uuid::new_v4().to_string();
        let storage_path = format!("attachments/{storage_filename}");

        let mimetype = content_type.clone();
        let original_filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_attachment".to_string());

        let file_size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!("Failed to stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        crash_info["attachments"]
            .as_array_mut()
            .ok_or_else(|| {
                error!("Crash info attachments is not an array");
                ApiError::Failure("crash info attachments is not an array".to_string())
            })?
            .push(serde_json::json!({
                "filename": original_filename,
                "mimetype": mimetype,
                "size": file_size,
                "storage_path": storage_path,
                "storage_filename": storage_filename,

            }));

        Ok(())
    }

    async fn handle_annotation_upload(
        field: Field<'_>,
        crash_info: &mut serde_json::Value,
    ) -> Result<(), ApiError> {
        let key = field
            .name()
            .ok_or_else(|| {
                error!("Field name is missing");
                ApiError::Failure("field name is missing".to_string())
            })?
            .to_string();
        Self::validate_key(&key)?;

        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_content_type(&content_type, "annotation")?;

        let value = field.text().await.map_err(|e| {
            error!("Failed to read field text for field '{}': {:?}", key, e);
            ApiError::Failure(format!("failed to read field text for field '{key}': {e:?}"))
        })?;

        match key.as_str() {
            key if Self::REQUIRED_FIELDS.contains(&key) => {
                crash_info[key] = serde_json::json!(value);
            }
            _ => {
                crash_info["annotations"]
                    .as_object_mut()
                    .ok_or_else(|| {
                        error!("Crash info is not an object");
                        ApiError::Failure("crash info is not an object".to_string())
                    })?
                    .insert(key.clone(), serde_json::json!(value));
            }
        }

        Ok(())
    }

    async fn process_field(
        field: Field<'_>,
        crash_info: &mut serde_json::Value,
        state: AppState,
    ) -> Result<(), ApiError> {
        let content_type = field.content_type().unwrap_or_default().to_owned();
        match content_type.as_str() {
            "application/octet-stream" if field.name() == Some("upload_file_minidump") => {
                Self::handle_minidump_upload(field, crash_info, state).await
            }
            "application/octet-stream" => {
                Self::handle_attachment_upload(field, crash_info, state).await
            }
            _ => Self::handle_annotation_upload(field, crash_info).await,
        }
    }

    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        WithRejection(Query(params), _): WithRejection<Query<MinidumpRequestParams>, ApiError>,
        mut multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let crash_id = uuid::Uuid::new_v4();
        let mut crash_info = serde_json::json!({
            "crash_id": crash_id,
            "submission_timestamp": chrono::Utc::now().to_rfc3339(),
            "attachments": [],
            "annotations": {},
        });
        params.validate()?;

        let mut tx = state.repo.begin_admin().await?;

        let product_id = api_token.product_id.ok_or_else(|| {
            error!("API token does not have a product ID");
            ApiError::ProductAccessDenied(
                "API token is not associated with any product".to_string(),
            )
        })?;

        let authorized_product = get_product_by_id(&mut *tx, product_id).await?;
        crash_info["authorized_product"] = serde_json::json!(authorized_product.name);

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("Failed to get next multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, &mut crash_info, state.clone()).await?;
        }

        if crash_info.get("minidump").is_none_or(|v| v.is_null()) {
            return Err(ApiError::Failure("no minidump found in submission".to_string()));
        }

        for &required_field in Self::REQUIRED_FIELDS {
            if crash_info.get(required_field).is_none_or(|v| v.is_null()) {
                return Err(ApiError::Failure(format!(
                    "required annotation '{required_field}' is missing"
                )));
            } else if let Some(value) = crash_info.get(required_field) {
                if value.is_string() && value.as_str().map(|s| s.trim().is_empty()).unwrap_or(false)
                {
                    return Err(ApiError::Failure(format!(
                        "required annotation '{required_field}' cannot be empty"
                    )));
                }
            }
        }

        let product_name = crash_info["product"].as_str().ok_or_else(|| {
            error!("No product found");
            ApiError::Failure("no product found".to_string())
        })?;
        let product = get_product(&mut *tx, product_name).await?;
        validate_api_token_for_product(&api_token, &product, product_name)?;

        state.repo.end(tx).await?;

        let path = Path::from(format!("crashes/{crash_id}"));
        let crash_json = crash_info.to_string();
        let payload = PutPayload::from(crash_json.into_bytes());
        state.storage.put(&path, payload).await.map_err(|e| {
            error!("Failed to upload crash info to S3: {:?}", e);
            ApiError::Failure("failed to upload crash info to S3".to_string())
        })?;

        state.worker.queue_minidump(crash_info).await.map_err(|e| {
            error!("Failed to queue minidump job: {:?}", e);
            ApiError::Failure("failed to queue minidump job".to_string())
        })?;

        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
            crash_id: Some(crash_id),
        }))
    }
}
