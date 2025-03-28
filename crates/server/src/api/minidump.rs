use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::{Symbolizer, simple_symbol_supplier};
use repos::attachment::AttachmentRepo;
use repos::crash::CrashRepo;
use repos::product::ProductRepo;
use repos::version::VersionRepo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Postgres;
use std::path::PathBuf;
use tokio::task;
use tracing::{debug, error};

use super::error::ApiError;
use super::stream_to_file;
use crate::app_state::AppState;
use crate::settings;

pub struct MinidumpApi;

#[derive(Debug, Deserialize)]
pub struct MinidumpRequestParams {
    pub product: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
}

impl MinidumpApi {
    async fn get_minidump_file(name: String) -> Result<PathBuf, ApiError> {
        let upload_path = std::path::Path::new(&settings().server.base_path).join("minidumps");
        let minidump_file = std::path::Path::new(&upload_path).join(name);
        tokio::fs::create_dir_all(&upload_path).await?;
        Ok(minidump_file)
    }

    async fn get_attachment_file(crash: uuid::Uuid, name: String) -> Result<PathBuf, ApiError> {
        let upload_path = std::path::Path::new(&settings().server.base_path)
            .join("attachments")
            .join(crash.to_string());
        let minidump_file = std::path::Path::new(&upload_path).join(name);
        tokio::fs::create_dir_all(&upload_path).await?;
        Ok(minidump_file)
    }

    async fn store_crash(
        tx: impl sqlx::Executor<'_, Database = Postgres>,
        report: serde_json::Value,
        product: repos::product::Product,
        version: repos::version::Version,
    ) -> Result<uuid::Uuid, ApiError> {
        let dto = repos::crash::NewCrash {
            report, //: report, // TODO: .to_string(),
            summary: "".to_string(),
            product_id: product.id,
            version_id: version.id,
        };
        let id = CrashRepo::create(tx, dto).await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;
        Ok(id)
    }

    async fn store_attachment(
        tx: impl sqlx::Executor<'_, Database = Postgres>,
        product_id: uuid::Uuid,
        crash_id: uuid::Uuid,
        filename: String,
        filesize: i64,
        mime_type: String,
    ) -> Result<uuid::Uuid, ApiError> {
        let dto = repos::attachment::NewAttachment {
            name: "minidump".to_string(),
            mime_type,
            size: filesize,
            filename,
            crash_id,
            product_id,
        };
        let id = AttachmentRepo::create(tx, dto).await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;
        Ok(id)
    }

    async fn process_minidump_file(minidump_file: PathBuf) -> Result<serde_json::Value, ApiError> {
        debug!("minidump_file: {:?}", minidump_file);
        let dump = Minidump::read_path(minidump_file)?;

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let path = std::path::Path::new(&settings().server.base_path)
            .join("symbols")
            .to_path_buf();
        debug!("provider: {:?}", path);
        let provider = Symbolizer::new(simple_symbol_supplier(vec![path]));

        let state =
            minidump_processor::process_minidump_with_options(&dump, &provider, options).await?;

        let mut json_output = Vec::new();
        state.print_json(&mut json_output, false)?;
        let json: Value = serde_json::from_slice(&json_output)?;

        debug!("json: {:?}", json);
        Ok(json)
    }

    async fn handle_minidump_upload(
        state: &AppState,
        params: &MinidumpRequestParams,
        field: Field<'_>,
    ) -> Result<uuid::Uuid, ApiError> {
        let mut tx = state.repo.begin_admin().await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let minidump_file = Self::get_minidump_file(filename).await?;

        let product = ProductRepo::get_by_name(&mut *tx, &params.product)
            .await?
            .ok_or(ApiError::Failure)?;

        let version = VersionRepo::get_by_product_and_name(&mut *tx, product.id, &params.version)
            .await?
            .ok_or(ApiError::Failure)?;

        stream_to_file(&minidump_file, field).await?;

        let data = task::spawn_blocking(move || Self::process_minidump_file(minidump_file))
            .await?
            .await?;

        let crash_id = Self::store_crash(&mut *tx, data, product, version).await?;

        tx.commit().await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;
        Ok(crash_id)
    }

    async fn handle_attachment_upload(
        crash_id: uuid::Uuid,
        state: &AppState,
        _params: &MinidumpRequestParams,
        field: Field<'_>,
    ) -> Result<(), ApiError> {
        let mut tx = state.repo.acquire_admin().await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let attachment_file = Self::get_attachment_file(crash_id, filename).await?;

        let mimetype = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        stream_to_file(&attachment_file, field).await?;

        let crash = CrashRepo::get_by_id(&mut *tx, crash_id)
            .await?
            .ok_or(ApiError::Failure)?;

        Self::store_attachment(
            &mut *tx,
            crash.id,
            crash_id,
            attachment_file
                .to_str()
                .ok_or(ApiError::Failure)?
                .to_string(),
            0, // TODO: compute filesize
            mimetype,
        )
        .await?;

        Ok(())
    }

    pub async fn upload(
        State(state): State<AppState>,
        Query(params): Query<MinidumpRequestParams>,
        mut multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let mut crash_id: Option<uuid::Uuid> = None;

        while let Some(field) = multipart.next_field().await? {
            match field.name() {
                Some("upload_file_minidump") => {
                    crash_id = Some(Self::handle_minidump_upload(&state, &params, field).await?)
                }
                Some("options") => {
                    let _content = field.bytes().await?;
                }
                Some(_) => {
                    Self::handle_attachment_upload(
                        crash_id.ok_or(ApiError::Failure)?,
                        &state,
                        &params,
                        field,
                    )
                    .await?
                }
                _ => (),
            }
        }
        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
        }))
    }
}
