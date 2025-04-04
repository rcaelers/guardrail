use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use minidump::Minidump;
use minidump_processor::ProcessorOptions;
use minidump_unwind::{Symbolizer, simple_symbol_supplier};
use repos::attachment::AttachmentRepo;
use repos::crash::CrashRepo;
use repos::product::{Product, ProductRepo};
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
    async fn get_minidump_file(name: &String) -> Result<PathBuf, ApiError> {
        let upload_path = std::path::Path::new(&settings().server.base_path).join("minidumps");
        let minidump_file = std::path::Path::new(&upload_path).join(name);
        tokio::fs::create_dir_all(&upload_path).await.map_err(|e| {
            error!(
                "failed to create directories {} for storing minidump {} ({:?})",
                upload_path.to_str().unwrap_or("?"),
                name,
                e
            );
            ApiError::Failure(format!("failed to store minidump {}", name))
        })?;
        Ok(minidump_file)
    }

    async fn get_attachment_file(crash: uuid::Uuid, name: &String) -> Result<PathBuf, ApiError> {
        let upload_path = std::path::Path::new(&settings().server.base_path)
            .join("attachments")
            .join(crash.to_string());
        let attachment_file = std::path::Path::new(&upload_path).join(name);
        tokio::fs::create_dir_all(&upload_path).await.map_err(|e| {
            error!(
                "failed to create directories {} for storing attachment {} ({:?})",
                name,
                upload_path.to_str().unwrap_or("?"),
                e
            );
            ApiError::Failure(format!("failed to store attachment {}", name))
        })?;
        Ok(attachment_file)
    }

    async fn store_crash<E>(
        tx: &mut E,
        report: serde_json::Value,
        product: &repos::product::Product,
        version: &repos::version::Version,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let crash = repos::crash::NewCrash {
            report,
            summary: "".to_string(),
            product_id: product.id,
            version_id: version.id,
        };
        let id = CrashRepo::create(&mut *tx, crash).await.map_err(|e| {
            error!("failed to store crassh report for {}/{} ({:?})", product.name, version.name, e);
            ApiError::Failure(format!("failed to store crash report",))
        })?;
        Ok(id)
    }

    async fn store_attachment<E>(
        tx: &mut E,
        product: &repos::product::Product,
        crash: &repos::crash::Crash,
        filename: String,
        filesize: i64,
        mime_type: String,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let attachment = repos::attachment::NewAttachment {
            name: filename.clone(),
            mime_type,
            size: filesize,
            filename: filename.clone(),
            crash_id: crash.id,
            product_id: product.id,
        };
        let id = AttachmentRepo::create(&mut *tx, attachment)
            .await
            .map_err(|e| {
                error!(
                    "failed to store attachment {} for {}/{} ({:?})",
                    filename.clone(),
                    product.name,
                    crash.id,
                    e
                );
                ApiError::Failure(format!("failed to store attachment {}", filename))
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
        state.print_json(&mut json_output, false).map_err(|e| {
            error!("failed to print minidump json: {:?}", e);
            ApiError::Failure("failed to print minidump json".to_string())
        })?;
        let json: Value = serde_json::from_slice(&json_output).map_err(|e| {
            error!("failed to parse minidump json: {:?}", e);
            ApiError::Failure("failed to parse minidump json".to_string())
        })?;

        debug!("json: {:?}", json);
        Ok(json)
    }

    async fn handle_minidump_upload<E>(
        tx: &mut E,
        product: &Product,
        params: &MinidumpRequestParams,
        field: Field<'_>,
    ) -> Result<uuid::Uuid, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let minidump_file = Self::get_minidump_file(&filename).await?;

        let version = VersionRepo::get_by_product_and_name(&mut *tx, product.id, &params.version)
            .await
            .map_err(|_| {
                error!("failed to get version for {}/{}", product.name, params.version);
                ApiError::Failure(format!(
                    "failed to get version for {}/{}",
                    product.name, params.version
                ))
            })?
            .ok_or_else(|| {
                error!("no such version for {}/{}", product.name, params.version);
                ApiError::VersionNotFound(product.name.clone(), params.version.clone())
            })?;

        stream_to_file(&minidump_file, field).await?;

        let data = task::spawn(async move { Self::process_minidump_file(minidump_file).await })
            .await
            .map_err(|e| {
                error!("failed to process minidump file: {:?}", e);
                ApiError::Failure("failed to process minidump file".to_string())
            })?
            .map_err(|e| {
                error!("failed to process minidump file: {:?}", e);
                ApiError::Failure("failed to process minidump file".to_string())
            })?;

        let crash_id = Self::store_crash(tx, data, product, &version).await?;

        Ok(crash_id)
    }

    async fn handle_attachment_upload<E>(
        tx: &mut E,
        crash_id: uuid::Uuid,
        product: &Product,
        field: Field<'_>,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let attachment_file = Self::get_attachment_file(crash_id, &filename).await?;

        let mimetype = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        stream_to_file(&attachment_file, field).await?;

        let crash = CrashRepo::get_by_id(&mut *tx, crash_id)
            .await
            .map_err(|_| {
                error!("failed to get crash {}", crash_id);
                ApiError::Failure(format!("failed to get crash {}", crash_id))
            })?
            .ok_or_else(|| {
                error!("No such crash {}", crash_id);
                ApiError::CrashNotFound()
            })?;

        Self::store_attachment(
            tx, product, &crash, filename, 0, // TODO: compute filesize
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

        let mut tx = state.repo.begin_admin().await.map_err(|e| {
            error!("failed to start transaction: {:?}", e);
            ApiError::Failure("failed to start transaction".to_string())
        })?;

        let product = ProductRepo::get_by_name(&mut *tx, &params.product)
            .await
            .map_err(|_| {
                error!("failed to get product {}", params.product);
                ApiError::Failure(format!("failed to get product {}", params.product))
            })?
            .ok_or_else(|| {
                error!("No such product {}", params.product);
                ApiError::ProductNotFound(params.product.clone())
            })?;

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("failed to read multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            match field.name() {
                Some("upload_file_minidump") => {
                    crash_id = Some(
                        Self::handle_minidump_upload(&mut *tx, &product, &params, field).await?,
                    )
                }
                Some("options") => {
                    let _content = field.bytes().await.map_err(|e| {
                        error!("failed to read options field: {:?}", e);
                        ApiError::Failure("failed to read options field".to_string())
                    })?;
                }
                Some(_) => {
                    Self::handle_attachment_upload(
                        &mut *tx,
                        crash_id.ok_or(ApiError::Failure(
                            "Expect crash before atttachment".to_string(),
                        ))?,
                        &product,
                        field,
                    )
                    .await?
                }
                _ => (),
            }
        }

        tx.commit().await.map_err(|e| {
            error!("failed to commit transaction: {:?}", e);
            ApiError::Failure("failed to commit transaction".to_string())
        })?;

        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
        }))
    }
}
