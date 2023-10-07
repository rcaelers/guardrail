use axum::body::Bytes;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{BoxError, Json};
use futures::prelude::*;
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::{simple_symbol_supplier, Symbolizer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;
use tracing::{debug, error, info};

use tokio::task;

use super::error::ApiError;
use crate::app_state::AppState;
use crate::model::attachment::{AttachmentDto, AttachmentRepo};
use crate::model::base::{BaseRepo, BaseRepoWithSecondaryKey};
use crate::model::crash::{CrashDto, CrashRepo};
use crate::model::product::ProductRepo;
use crate::model::version::VersionRepo;
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
    async fn stream_to_file<S, E>(path: &std::path::PathBuf, stream: S) -> Result<(), ApiError>
    where
        S: Stream<Item = Result<Bytes, E>>,
        E: Into<BoxError>,
    {
        async {
            let body_with_io_error =
                stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
            let body_reader = StreamReader::new(body_with_io_error);
            futures::pin_mut!(body_reader);

            let mut file = BufWriter::new(File::create(path).await?);
            let r = tokio::io::copy(&mut body_reader, &mut file).await;
            info!("r: {:?}", r);

            Ok::<(), ApiError>(())
        }
        .await
        .map_err(|_err| (ApiError::Failure))
    }

    async fn get_product(
        state: &Arc<AppState>,
        params: &MinidumpRequestParams,
    ) -> Result<crate::model::product::Product, ApiError> {
        let product = ProductRepo::get_by_secondary_id(&state.db, params.product.clone()).await;
        let product = match product {
            Ok(product) => product,
            Err(e) => {
                error!("error: {:?}", e);
                return Err(ApiError::Failure);
            }
        }
        .ok_or(ApiError::Failure)?;
        info!("product: {:?}", product.id);
        Ok(product)
    }

    async fn get_version(
        state: &Arc<AppState>,
        product_id: uuid::Uuid,
        params: &MinidumpRequestParams,
    ) -> Result<crate::model::version::Version, ApiError> {
        let version =
            VersionRepo::get_by_product_and_name(&state.db, product_id, params.version.clone())
                .await;
        let version = match version {
            Ok(product) => product,
            Err(e) => {
                error!("error: {:?}", e);
                return Err(ApiError::Failure);
            }
        }
        .ok_or(ApiError::Failure)?;
        info!("version: {:?}", version.id);
        Ok(version)
    }

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
        report: serde_json::Value,
        product: crate::model::product::Product,
        version: crate::model::version::Version,
        state: Arc<AppState>,
    ) -> Result<uuid::Uuid, ApiError> {
        let dto = CrashDto {
            report: report.to_string(),
            product_id: product.id,
            version_id: version.id,
        };
        let id = CrashRepo::create(&state.db, dto).await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;
        Ok(id)
    }

    async fn store_attachment(
        crash_id: uuid::Uuid,
        filename: String,
        filesize: i64,
        mime_type: String,
        state: Arc<AppState>,
    ) -> Result<uuid::Uuid, ApiError> {
        let dto = AttachmentDto {
            name: "minidump".to_string(),
            mime_type,
            size: filesize,
            filename,
            crash_id,
        };
        let id = AttachmentRepo::create(&state.db, dto).await.map_err(|e| {
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
        state: Arc<AppState>,
        params: &MinidumpRequestParams,
        field: Field<'_>,
    ) -> Result<uuid::Uuid, ApiError> {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let minidump_file = Self::get_minidump_file(filename).await?;

        let product = Self::get_product(&state, params).await?;
        let version = Self::get_version(&state, product.id, params).await?;

        Self::stream_to_file(&minidump_file, field).await?;

        let data = task::spawn_blocking(move || Self::process_minidump_file(minidump_file))
            .await?
            .await?;

        let crash_id = Self::store_crash(data, product, version, state).await?;

        Ok(crash_id)
    }

    async fn handle_attachment_upload(
        crash_id: uuid::Uuid,
        state: Arc<AppState>,
        params: &MinidumpRequestParams,
        field: Field<'_>,
    ) -> Result<(), ApiError> {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let attachment_file = Self::get_attachment_file(crash_id, filename).await?;

        let mimetype = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        Self::stream_to_file(&attachment_file, field).await?;

        Self::store_attachment(
            crash_id,
            attachment_file
                .to_str()
                .ok_or(ApiError::Failure)?
                .to_string(),
            0, // TODO: compute filesize
            mimetype,
            state,
        )
        .await?;

        Ok(())
    }

    pub async fn upload(
        State(state): State<Arc<AppState>>,
        Query(params): Query<MinidumpRequestParams>,
        mut multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let mut crash_id: Option<uuid::Uuid> = None;

        while let Some(field) = multipart.next_field().await? {
            match field.name() {
                Some("upload_file_minidump") => {
                    crash_id = Some(
                        Self::handle_minidump_upload(Arc::clone(&state), &params, field).await?,
                    )
                }
                Some("options") => {
                    let content = field.bytes().await?;
                    info!("options: {:?}", content);
                }
                Some(_) => {
                    Self::handle_attachment_upload(
                        crash_id.ok_or(ApiError::Failure)?,
                        Arc::clone(&state),
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
