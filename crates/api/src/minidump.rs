use axum::extract::multipart::Field;
use axum::extract::{Multipart, State};
use axum::{Extension, Json};
use object_store::path::Path;
use object_store::{ObjectStore, PutPayload};
use serde::Serialize;
use sqlx::Postgres;
use std::sync::Arc;
use tracing::{error, info, instrument};

use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, get_product_by_id, stream_to_s3, validate_api_token_for_product};
use data::api_token::ApiToken;

pub struct MinidumpApi;

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
    pub crash_id: Option<uuid::Uuid>,
}

#[derive(Default, Debug, Serialize)]
struct Minidump {
    filename: String,
    content_type: String,
    size: u64,
    storage_path: String,
    storage_filename: String,
}

#[derive(Default, Debug, Serialize)]
struct Attachment {
    name: String,
    filename: String,
    content_type: String,
    size: u64,
    storage_path: String,
    storage_filename: String,
}

#[derive(Default, Debug, Serialize)]
struct CrashInfo {
    crash_id: uuid::Uuid,
    submission_timestamp: String,
    authorized_product: Option<String>,
    product: Option<String>,
    version: Option<String>,
    channel: Option<String>,
    commit: Option<String>,
    build_id: Option<String>,
    minidump: Option<Minidump>,
    attachments: Vec<Attachment>,
    annotations: std::collections::HashMap<String, String>,
}

impl MinidumpApi {
    const REQUIRED_FIELDS: &'static [&'static str] =
        &["product", "version", "channel", "commit", "build_id"];

    fn validate_annotation_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "text/plain"
            || content_type == "text/markdown"
            || content_type.is_empty();

        if !is_valid {
            error!(content_type, "Invalid annotation content type");
            return Err(ApiError::Failure(format!(
                "invalid annotation content type: {content_type}"
            )));
        }
        Ok(())
    }

    fn validate_key(key: &str) -> Result<(), ApiError> {
        if !key.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            error!(key, "Annotation key contains non-printable ASCII characters");
            return Err(ApiError::Failure(
                "annotation key must contain only printable ASCII characters".to_string(),
            ));
        }
        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn handle_minidump_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        info!("Processing minidump");
        let content_type = field.content_type().unwrap_or_default().to_owned();

        let storage_filename = uuid::Uuid::new_v4().to_string();
        let storage_path = format!("minidumps/{storage_filename}");

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_minidump".to_string());

        let size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to stream minidump to S3");
                ApiError::Failure("failed to store minidump".to_string())
            })?;

        crash_info.minidump = Some(Minidump {
            filename,
            content_type,
            size,
            storage_path,
            storage_filename,
        });

        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn handle_attachment_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        info!("Processing attachment");
        let content_type = field.content_type().unwrap_or_default().to_owned();

        let storage_filename = uuid::Uuid::new_v4().to_string();
        let storage_path = format!("attachments/{storage_filename}");

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_attachment".to_string());

        let name = field
            .name()
            .ok_or_else(|| {
                error!("Name field is missing for attachment");
                ApiError::Failure("name field for attachment is missing".to_string())
            })?
            .to_string();

        let size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!(error = ?e, attachment_name = name, "Failed to stream attachment to S3");
                ApiError::Failure("failed to store attachment".to_string())
            })?;

        crash_info.attachments.push(Attachment {
            name,
            filename,
            content_type,
            size,
            storage_path,
            storage_filename,
        });
        Ok(())
    }

    #[instrument(skip(field, crash_info), fields(crash_id = %crash_info.crash_id))]
    async fn handle_annotation_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        info!("Processing annotation");
        let key = field
            .name()
            .ok_or_else(|| {
                error!("Name field is missing for annotation");
                ApiError::Failure("name field is missing for annotation".to_string())
            })?
            .to_string();
        Self::validate_key(&key)?;

        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_annotation_content_type(&content_type)?;

        let value = field.text().await.map_err(|e| {
            error!(error = ?e, key, "Failed to read field text for annotation");
            ApiError::Failure(format!("failed to read field text for annotation '{key}'"))
        })?;

        crash_info.annotations.insert(key, value);
        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn process_field(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        let content_type = field.content_type().unwrap_or_default().to_owned();
        let field_name = field.name().unwrap_or_default();

        match content_type.as_str() {
            "application/octet-stream" if field_name == "upload_file_minidump" => {
                Self::handle_minidump_upload(field, crash_info, state).await
            }
            "application/octet-stream" => {
                Self::handle_attachment_upload(field, crash_info, state).await
            }
            _ => Self::handle_annotation_upload(field, crash_info).await,
        }
    }

    #[instrument(skip(tx, api_token, crash_info), fields(crash_id = %crash_info.crash_id))]
    async fn validate_crash<E>(
        tx: &mut E,
        api_token: &ApiToken,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        if crash_info.minidump.is_none() {
            error!("No minidump found in submission");
            return Err(ApiError::Failure("no minidump found in submission".to_string()));
        }

        let mut build_timestamp = chrono::Utc::now();
        let mut channel = String::new();
        let mut product_name = String::new();
        let mut version = String::new();
        for &required_field in Self::REQUIRED_FIELDS {
            let value = crash_info.annotations.get(required_field).cloned();
            crash_info.annotations.remove(required_field);

            if value.is_none() {
                error!(required_field, "Required annotation is missing");
                return Err(ApiError::Failure(format!(
                    "required annotation '{required_field}' is missing"
                )));
            }

            if let Some(value) = value {
                if value.trim().is_empty() {
                    error!(required_field, "Required annotation is empty");
                    return Err(ApiError::Failure(format!(
                        "required annotation '{required_field}' cannot be empty"
                    )));
                }

                match required_field {
                    "product" => {
                        let product = get_product(tx, &value).await?;
                        validate_api_token_for_product(api_token, &product, &value)?;
                        if !product.accepting_crashes {
                            return Err(ApiError::ProductNotAcceptingCrashes(value));
                        }
                        product_name = product.name;
                        crash_info.product = Some(product_name.clone());
                    }
                    "build_id" => {
                        build_timestamp =
                            value
                                .parse::<chrono::DateTime<chrono::Utc>>()
                                .map_err(|e| {
                                    error!(error = ?e, "Invalid build timestamp");
                                    ApiError::Failure("invalid build timestamp".to_string())
                                })?;
                        crash_info.build_id = Some(value);
                    }
                    "channel" => {
                        channel = value;
                        crash_info.channel = Some(channel.clone());
                    }
                    "version" => {
                        version = value;
                        crash_info.version = Some(version.clone());
                    }
                    "commit" => {
                        crash_info.commit = Some(value);
                    }
                    _ => {}
                }
            }
        }

        if channel != "release"
            && build_timestamp < chrono::Utc::now() - chrono::Duration::days(365 * 2)
        {
            return Err(ApiError::TooOld(version, product_name));
        }
        Ok(())
    }

    #[instrument(skip(storage, crash_info), fields(crash_id = %crash_id))]
    async fn upload_crash(
        storage: Arc<dyn ObjectStore>,
        crash_id: uuid::Uuid,
        crash_info: serde_json::Value,
    ) -> Result<(), ApiError> {
        let path = Path::from(format!("crashes/{crash_id}.json"));
        let crash_info_json = crash_info.to_string();
        let payload = PutPayload::from(crash_info_json.into_bytes());
        storage.put(&path, payload).await.map_err(|e| {
            error!(error = ?e, "Failed to upload crash info to S3");
            ApiError::Failure("failed to upload crash info to S3".to_string())
        })?;
        Ok(())
    }

    #[instrument(skip(state, api_token, multipart), fields(crash_id))]
    pub async fn handle_upload(
        state: AppState,
        api_token: ApiToken,
        mut multipart: Multipart,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        let mut tx = state.repo.begin_admin().await?;

        let product_id = api_token.product_id.ok_or_else(|| {
            error!("API token does not have a product ID");
            ApiError::ProductAccessDenied(
                "API token is not associated with any product".to_string(),
            )
        })?;

        let authorized_product = get_product_by_id(&mut *tx, product_id).await?;
        crash_info.authorized_product = Some(authorized_product.name.clone());

        info!(product = %authorized_product.name, "Processing crash for product");

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!(error = ?e, "Failed to get next multipart field");
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, crash_info, state.clone()).await?;
        }

        Self::validate_crash(&mut *tx, &api_token, crash_info).await?;

        state.repo.end(tx).await?;

        let crash_info_json = serde_json::to_value(&crash_info).map_err(|e| {
            error!(error = ?e, "Failed to serialize crash info");
            ApiError::Failure("failed to serialize crash info".to_string())
        })?;

        Self::upload_crash(state.storage.clone(), crash_info.crash_id, crash_info_json.clone()).await?;


        state
            .worker
            .queue_minidump(crash_info_json)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to queue minidump job");
                ApiError::Failure("failed to queue minidump job".to_string())
            })?;

        Ok(())
    }

    #[instrument(skip(state, api_token, multipart), fields(crash_id))]
    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let crash_id = uuid::Uuid::new_v4();
        tracing::Span::current().record("crash_id", format!("{crash_id}"));

        let mut crash_info = CrashInfo {
            crash_id,
            submission_timestamp: chrono::Utc::now().to_rfc3339(),
            authorized_product: None,
            minidump: None,
            attachments: Vec::new(),
            annotations: std::collections::HashMap::new(),
            ..Default::default()
        };

        let r = Self::handle_upload(state.clone(), api_token, multipart, &mut crash_info).await;
        if let Err(e) = r {
            error!(error = ?e, "Failed to handle minidump upload");

            if let Some(minidump) = &crash_info.minidump {
                info!(storage_path = %minidump.storage_path, "Deleting minidump from storage");
                let _ = state.storage.delete(&Path::from(minidump.storage_path.as_str())).await;
            }

            for attachment in &crash_info.attachments {
                info!(storage_path = %attachment.storage_path, "Deleting attachment from storage");
                let _ = state.storage.delete(&Path::from(attachment.storage_path.as_str())).await;
            }

            info!(crash_id = %crash_info.crash_id, "Deleting crash info from storage");
            let _ = state.storage.delete(&Path::from(format!("crashes/{}.json", crash_info.crash_id))).await;
            return Err(e);
        }

        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
            crash_id: Some(crash_id),
        }))
    }
}
