use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use axum_extra::extract::WithRejection;
use data::api_token::ApiToken;
use data::attachment::NewAttachment;
use data::product::Product;
use data::version::Version;
use repos::attachment::AttachmentsRepo;
use repos::crash::CrashRepo;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use tracing::{error, info};

use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, get_version, stream_to_s3, validate_api_token_for_product};

pub struct MinidumpApi;

#[derive(Debug, Deserialize)]
pub struct MinidumpRequestParams {
    pub product: String,
    pub version: String,
}

impl MinidumpRequestParams {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.product.trim().is_empty() {
            return Err(ApiError::Failure("product name cannot be empty".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(ApiError::Failure("version cannot be empty".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
    pub crash_id: Option<uuid::Uuid>,
}

impl MinidumpApi {
    fn audit_log(
        event: &str,
        details: &str,
        product: Option<&str>,
        version: Option<&str>,
        crash_id: Option<uuid::Uuid>,
    ) {
        let product_info = product.map_or("unknown".to_string(), |p| p.to_string());
        let version_info = version.map_or("unknown".to_string(), |v| v.to_string());
        let crash_info = crash_id.map_or("none".to_string(), |id| id.to_string());

        info!(
            event = event,
            product = product_info,
            version = version_info,
            crash_id = crash_info,
            details = details,
            "AUDIT: {}: {} (product: {}, version: {}, crash: {})",
            event,
            details,
            product_info,
            version_info,
            crash_info
        );
    }

    fn validate_content_type(
        content_type: &str,
        content_type_category: &str,
    ) -> Result<(), ApiError> {
        let is_valid = match content_type_category {
            "minidump" => {
                matches!(
                    content_type,
                    "application/octet-stream"
                        | "application/x-dmp"
                        | "application/x-minidump"
                        | "" // Accept empty content type for compatibility
                )
            }
            "attachment" => {
                !content_type.contains("text/html")
                    && !content_type.contains("application/javascript")
            }
            _ => false,
        };

        if !is_valid {
            error!("Invalid {} content type: {}", content_type_category, content_type);
            return Err(ApiError::Failure(format!(
                "invalid {} content type: {}",
                content_type_category, content_type
            )));
        }
        Ok(())
    }

    fn validate_attachment_content_type(content_type: &str) -> Result<(), ApiError> {
        Self::validate_content_type(content_type, "attachment")
    }

    fn validate_minidump_content_type(content_type: &str) -> Result<(), ApiError> {
        Self::validate_content_type(content_type, "minidump")
    }

    fn extract_filename(field: &Field<'_>) -> Result<String, ApiError> {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        Ok(filename)
    }

    async fn handle_minidump_upload<E>(
        tx: &mut E,
        product: &Product,
        version: &Version,
        field: Field<'_>,
        state: AppState,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        Self::audit_log(
            "minidump_upload_start",
            "Processing minidump upload",
            Some(&product.name),
            Some(&version.name),
            None,
        );

        let content_type = field.content_type().unwrap_or_default();
        Self::validate_minidump_content_type(content_type)?;

        let _filename = Self::extract_filename(&field)?;

        let minidump = uuid::Uuid::new_v4();
        let path = format!("minidumps/{}", minidump);

        stream_to_s3(state.storage.clone(), &path, field)
            .await
            .map_err(|e| {
                error!("Failed to stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        let crash = data::crash::NewCrash {
            minidump,
            info: None,
            product_id: product.id,
            version_id: version.id,
        };
        let id = CrashRepo::create(&mut *tx, crash).await?;

        Self::audit_log(
            "minidump_file_saved",
            &format!("Saved minidump file {}", path),
            Some(&product.name),
            Some(&version.name),
            None,
        );
        Ok(id)
    }

    async fn handle_attachment_upload<E>(
        tx: &mut E,
        crash_id: uuid::Uuid,
        product: &Product,
        _version: &Version,
        field: Field<'_>,
        state: AppState,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        Self::audit_log(
            "attachment_upload_start",
            "Processing attachment upload",
            Some(&product.name),
            None,
            Some(crash_id),
        );

        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();
        Self::validate_attachment_content_type(&content_type)?;

        let storage_filename = uuid::Uuid::new_v4().to_string();
        let path = format!("attachments/{}", storage_filename);
        let mimetype = content_type.clone();
        let original_filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_attachment".to_string());

        Self::audit_log(
            "attachment_details",
            &format!("attachment: {} ({})", original_filename, mimetype),
            Some(&product.name),
            None,
            Some(crash_id),
        );

        stream_to_s3(state.storage.clone(), &path, field)
            .await
            .map_err(|e| {
                error!("Failed to stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        let crash = CrashRepo::get_by_id(&mut *tx, crash_id)
            .await?
            .ok_or(ApiError::Failure("crash not found".to_string()))?;

        let attachment = NewAttachment {
            name: original_filename.clone(),
            mime_type: content_type.to_owned(),
            size: 0, // TODO: Add correct size
            filename: storage_filename.clone(),
            crash_id: crash.id,
            product_id: product.id,
        };
        AttachmentsRepo::create(&mut *tx, attachment).await?;

        Self::audit_log(
            "attachment_file_saved",
            &format!("Saved attachment file (storage name: {})", storage_filename),
            Some(&product.name),
            None,
            Some(crash_id),
        );

        Ok(())
    }

    async fn process_field<E>(
        tx: &mut E,
        field: Field<'_>,
        product: &Product,
        version: &Version,
        crash_id: &mut Option<uuid::Uuid>,
        state: AppState,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        match field.name() {
            Some("minidump_file") => {
                let new_crash_id =
                    Self::handle_minidump_upload(tx, product, version, field, state).await?;
                *crash_id = Some(new_crash_id);
                Ok(())
            }
            Some(_) => {
                let crash_id_value = crash_id
                    .ok_or(ApiError::Failure("expect crash as first document".to_string()))?;

                Self::handle_attachment_upload(tx, crash_id_value, product, version, field, state)
                    .await
            }
            _ => Ok(()),
        }
    }

    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        WithRejection(Query(params), _): WithRejection<Query<MinidumpRequestParams>, ApiError>,
        mut multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        Self::audit_log(
            "upload_start",
            &format!("Starting minidump upload process for {}/{}", params.product, params.version),
            Some(&params.product),
            Some(&params.version),
            None,
        );

        params.validate()?;

        let mut tx = state.repo.begin_admin().await?;

        let product = get_product(&mut *tx, &params.product).await?;
        validate_api_token_for_product(&api_token, &product, &params.product)?;
        let version = get_version(&mut *tx, &product, &params.version).await?;

        Self::audit_log(
            "processing_multipart",
            "Processing multipart form data",
            Some(&product.name),
            Some(&version.name),
            None,
        );

        let mut crash_id: Option<uuid::Uuid> = None;
        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("Failed to get next multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(&mut *tx, field, &product, &version, &mut crash_id, state.clone())
                .await?;
        }

        state.repo.end(tx).await?;

        if let Some(crash_id) = crash_id {
            state.worker.queue_minidump(crash_id).await.map_err(|e| {
                error!("Failed to queue minidump job: {:?}", e);
                ApiError::Failure("failed to queue minidump job".to_string())
            })?;
        }

        Self::audit_log(
            "upload_complete",
            &format!("Upload process completed successfully for {}/{}", product.name, version.name),
            Some(&product.name),
            Some(&version.name),
            crash_id,
        );

        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
            crash_id,
        }))
    }
}
