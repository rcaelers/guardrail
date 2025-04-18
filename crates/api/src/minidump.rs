use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use axum_extra::extract::WithRejection;
use data::api_token::ApiToken;
use data::crash::Crash;
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
            return Err(ApiError::Failure("Product name cannot be empty".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(ApiError::Failure("Version cannot be empty".to_string()));
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
                "Invalid {} content type: {}",
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

    async fn store_crash<E>(
        tx: &mut E,
        minidump: uuid::Uuid,
        product: &data::product::Product,
        version: &data::version::Version,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let crash = data::crash::NewCrash {
            minidump,
            info: None,
            product_id: product.id,
            version_id: version.id,
        };
        let id = CrashRepo::create(&mut *tx, crash).await.map_err(|e| {
            error!("Failed to store crash report for {}/{} ({:?})", product.name, version.name, e);
            ApiError::Failure("failed to store crash report".to_string())
        })?;
        Ok(id)
    }

    async fn store_attachment<E>(
        tx: &mut E,
        product: &data::product::Product,
        crash: &data::crash::Crash,
        filename: String,
        filesize: i64,
        mime_type: String,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let attachment = data::attachment::NewAttachment {
            name: filename.clone(),
            mime_type,
            size: filesize,
            filename: filename.clone(),
            crash_id: crash.id,
            product_id: product.id,
        };
        let id = AttachmentsRepo::create(&mut *tx, attachment)
            .await
            .map_err(|e| {
                error!(
                    "Failed to store attachment {} for {}/{} ({:?})",
                    filename.clone(),
                    product.name,
                    crash.id,
                    e
                );
                ApiError::Failure(format!("failed to store attachment {}", filename))
            })?;
        Ok(id)
    }

    async fn get_crash<E>(tx: &mut E, crash_id: uuid::Uuid) -> Result<Crash, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        CrashRepo::get_by_id(tx, crash_id)
            .await
            .map_err(|_| {
                error!("Failed to get crash {}", crash_id);
                ApiError::Failure(format!("failed to get crash {}", crash_id))
            })?
            .ok_or_else(|| {
                error!("No such crash {}", crash_id);
                ApiError::CrashNotFound()
            })
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

        let id = Self::store_crash(tx, minidump, product, version).await?;

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

        let content_type = field.content_type().unwrap_or("application/octet-stream");
        Self::validate_attachment_content_type(content_type)?;

        let mimetype = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        let original_filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_attachment".to_string());

        Self::audit_log(
            "attachment_details",
            &format!("Attachment: {} ({})", original_filename, mimetype),
            Some(&product.name),
            None,
            Some(crash_id),
        );

        let storage_filename = uuid::Uuid::new_v4().to_string();
        let path = format!("attachments/{}", storage_filename);

        stream_to_s3(state.storage.clone(), &path, field)
            .await
            .map_err(|e| {
                error!("Failed to stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        Self::audit_log(
            "attachment_file_saved",
            &format!("Saved attachment file (storage name: {})", storage_filename),
            Some(&product.name),
            None,
            Some(crash_id),
        );

        let crash = Self::get_crash(tx, crash_id).await?;
        let filesize = 0; // Placeholder for actual file size
        // TODO: add storage_filename
        Self::store_attachment(tx, product, &crash, original_filename, filesize, mimetype).await?;

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
            Some("upload_file_minidump") => {
                let new_crash_id =
                    Self::handle_minidump_upload(tx, product, version, field, state).await?;
                *crash_id = Some(new_crash_id);
                Ok(())
            }
            Some("options") => {
                let _content = field.bytes().await.map_err(|e| {
                    error!("Failed to read options field: {:?}", e);
                    ApiError::Failure("failed to read options field".to_string())
                })?;
                Ok(())
            }
            Some(_) => {
                let crash_id_value = crash_id
                    .ok_or(ApiError::Failure("Expect crash before attachment".to_string()))?;

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

        let mut tx = state.repo.begin_admin().await.map_err(|e| {
            error!("Failed to start transaction: {:?}", e);
            ApiError::Failure("failed to start transaction".to_string())
        })?;

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

        let commit_result = tx.commit().await;
        if let Err(e) = commit_result {
            error!("Failed to commit transaction: {:?}", e);
            return Err(ApiError::Failure("failed to commit transaction".to_string()));
        }

        if let Some(crash_id) = crash_id {
            state.worker.queue_minidump(crash_id).await.map_err(|e| {
                error!("Failed to queue minidump job: {:?}", e);
                ApiError::Failure("failed to queue minidump job".to_string())
            })?;
        } else {
            error!("No crash ID found after processing");
            return Err(ApiError::Failure("no crash ID found".to_string()));
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
