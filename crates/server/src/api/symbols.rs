use super::error::ApiError;
use super::file_cleanup::FileCleanupTracker;
use super::{get_product, get_version, validate_api_token_for_product, validate_file_size};
use crate::api::stream_to_file;
use crate::app_state::AppState;
use crate::settings;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use repos::api_token::ApiToken;
use repos::product::Product;
use repos::symbols::{NewSymbols, SymbolsRepo};
use repos::version::Version;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::path::PathBuf;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct SymbolsRequestParams {
    pub product: String,
    pub version: String,
}

impl SymbolsRequestParams {
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
pub struct SymbolsResponse {
    pub result: String,
}

#[derive(Clone, Debug, Serialize)]
struct SymbolsData {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
}

pub struct SymbolsApi;

impl SymbolsApi {
    fn audit_log(event: &str, details: &str, product: Option<&str>, version: Option<&str>) {
        let product_info = product.map_or("unknown".to_string(), |p| p.to_string());
        let version_info = version.map_or("unknown".to_string(), |v| v.to_string());

        info!(
            event = event,
            product = product_info,
            version = version_info,
            details = details,
            "AUDIT: {}: {} (product: {}, version: {})",
            event,
            details,
            product_info,
            version_info,
        );
    }

    fn validate_symbols_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "application/octet-stream"
            || content_type == "text/plain"
            || content_type.is_empty()  // Accept empty content type for compatibility
            || content_type.starts_with("text/");

        if !is_valid {
            error!("Invalid symbols content type: {}", content_type);
            return Err(ApiError::Failure(format!(
                "Invalid symbols content type: {}",
                content_type
            )));
        }
        Ok(())
    }

    fn get_max_symbols_size() -> u64 {
        const MAX_ATTACHMENT_SIZE: u64 = 10 * 1024 * 1024; // 10 MB default limit
        settings()
            .server
            .max_symbols_size
            .unwrap_or(MAX_ATTACHMENT_SIZE)
    }

    async fn get_temp_symbols_file() -> Result<PathBuf, ApiError> {
        let id = uuid::Uuid::new_v4();

        let upload_path = std::path::Path::new(&settings().server.base_path)
            .join("symbols")
            .join("tmp");
        let symbol_file = std::path::Path::new(&upload_path).join(id.to_string());
        tokio::fs::create_dir_all(&upload_path).await.map_err(|e| {
            error!("failed to create symbols upload directory {:?}: {:?}", upload_path, e);
            ApiError::InternalFailure()
        })?;
        Ok(symbol_file)
    }

    async fn get_header(symbol_file: &PathBuf) -> Result<String, ApiError> {
        let file = File::open(symbol_file).await.map_err(|e| {
            error!("failed to open symbols file {:?}: {:?}", symbol_file, e);
            ApiError::InternalFailure()
        })?;
        let mut reader = BufReader::new(file);
        let mut first_line = String::new();
        reader.read_line(&mut first_line).await.map_err(|e| {
            error!("failed to read header of symbols file {:?}: {:?}", symbol_file, e);
            ApiError::InternalFailure()
        })?;

        Ok(first_line)
    }

    fn validate_build_id(build_id: &str) -> Result<(), ApiError> {
        if build_id.is_empty() || build_id.len() > 64 {
            error!("Invalid build_id length: {}", build_id);
            return Err(ApiError::Failure("Invalid build_id length".to_string()));
        }

        if build_id.contains("..") || build_id.contains('/') || build_id.contains('\\') {
            error!("Invalid build_id contains path traversal characters: {}", build_id);
            return Err(ApiError::Failure(
                "Invalid build_id format (path traversal attempt)".to_string(),
            ));
        }

        if !build_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            error!("Invalid build_id format: {}", build_id);
            return Err(ApiError::Failure("Invalid build_id format".to_string()));
        }

        Ok(())
    }

    fn validate_module_id(module_id: &str) -> Result<(), ApiError> {
        if module_id.is_empty() || module_id.len() > 256 {
            error!("Invalid module_id length: {}", module_id);
            return Err(ApiError::Failure("Invalid module_id length".to_string()));
        }

        if module_id.contains("..") || module_id.contains('/') || module_id.contains('\\') {
            error!("Invalid module_id contains path traversal characters: {}", module_id);
            return Err(ApiError::Failure(
                "Invalid module_id format (path traversal attempt)".to_string(),
            ));
        }

        if !module_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        {
            error!("Invalid module_id format: {}", module_id);
            return Err(ApiError::Failure("Invalid module_id format".to_string()));
        }

        Ok(())
    }

    async fn process_symbol_file(
        symbol_file: &PathBuf,
        cleanup_tracker: &mut FileCleanupTracker,
    ) -> Result<SymbolsData, ApiError> {
        let first_line = Self::get_header(symbol_file).await?;

        let collection: Vec<&str> = first_line.split_whitespace().collect();
        if collection.len() < 5 {
            error!("invalid symbols file header: {:?}", first_line);
            return Err(ApiError::Failure("invalid symbols file header".to_string()));
        }
        let os = String::from(collection[1]);
        let arch = String::from(collection[2]);
        let build_id = String::from(collection[3]);
        let module_id = String::from(collection[4]);

        Self::validate_build_id(&build_id)?;
        Self::validate_module_id(&module_id)?;

        let final_path = std::path::Path::new(&settings().server.base_path)
            .join("symbols")
            .join(&module_id)
            .join(&build_id);

        tokio::fs::create_dir_all(&final_path).await.map_err(|e| {
            error!("failed to create symbols upload directory {:?}: {:?}", final_path, e);
            ApiError::InternalFailure()
        })?;

        let final_file = final_path.join(module_id.replace(".pdb", ".sym"));

        let r = SymbolsData {
            os,
            arch,
            build_id,
            module_id,
            file_location: final_file.to_str().unwrap_or("").to_string(),
        };

        cleanup_tracker.track_file(final_file.clone());

        fs::rename(&symbol_file, &final_file).await.map_err(|e| {
            error!("failed to rename symbols file {:?} to {:?}: {:?}", symbol_file, final_file, e);
            ApiError::InternalFailure()
        })?;

        Ok(r)
    }

    async fn store(
        tx: impl sqlx::Executor<'_, Database = Postgres>,
        data: SymbolsData,
        product: repos::product::Product,
        version: repos::version::Version,
    ) -> Result<(), ApiError> {
        let symbols = NewSymbols {
            os: data.os,
            arch: data.arch,
            build_id: data.build_id,
            module_id: data.module_id,
            file_location: data.file_location,
            product_id: product.id,
            version_id: version.id,
        };
        SymbolsRepo::create(tx, symbols).await.map_err(|e| {
            error!("failed to stored symbols {:?}", e);
            ApiError::InternalFailure()
        })?;
        Ok(())
    }

    async fn handle_symbol_upload<E>(
        tx: &mut E,
        product: &Product,
        version: &Version,
        field: Field<'_>,
        cleanup_tracker: &mut FileCleanupTracker,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        Self::audit_log(
            "symbol_upload_start",
            "Starting symbol file processing",
            Some(&product.name),
            Some(&version.name),
        );

        let content_type = field.content_type().unwrap_or_default();
        Self::validate_symbols_content_type(content_type)?;

        let symbol_file = Self::get_temp_symbols_file().await?;
        cleanup_tracker.track_file(symbol_file.clone());

        if let Err(e) = stream_to_file(&symbol_file, field).await {
            error!("Failed to save symbols file {:?}: {:?}", symbol_file, e);
            return Err(ApiError::InternalFailure());
        }

        let max_size = Self::get_max_symbols_size();
        let _filesize = validate_file_size(&symbol_file, max_size, "symbols").await? as i64;

        let data = Self::process_symbol_file(&symbol_file, cleanup_tracker).await?;

        Self::store(&mut *tx, data.clone(), product.clone(), version.clone()).await?;

        Self::audit_log(
            "symbol_upload_complete",
            &format!("Symbol file successfully processed and stored. Build ID: {}", data.build_id),
            Some(&product.name),
            Some(&version.name),
        );

        Ok(())
    }

    async fn process_field<E>(
        tx: &mut E,
        field: Field<'_>,
        product: &Product,
        version: &Version,
        cleanup_tracker: &mut FileCleanupTracker,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        match field.name() {
            Some("upload_file_symbols") => {
                Self::handle_symbol_upload(&mut *tx, product, version, field, cleanup_tracker)
                    .await?;
                Ok(())
            }
            Some("options") => {
                let _content = field.bytes().await.map_err(|e| {
                    error!("failed to read options field: {:?}", e);
                    ApiError::Failure("failed to read options field".to_string())
                })?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        Query(params): Query<SymbolsRequestParams>,
        mut multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        Self::audit_log(
            "symbols_upload_start",
            &format!("Starting symbols upload process for {}/{}", params.product, params.version),
            Some(&params.product),
            Some(&params.version),
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
        );

        let mut cleanup_tracker = FileCleanupTracker::new();

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("Failed to get next multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(&mut *tx, field, &product, &version, &mut cleanup_tracker).await?;
        }

        let commit_result = tx.commit().await;
        if let Err(e) = commit_result {
            error!("Failed to commit transaction: {:?}", e);
            cleanup_tracker.cleanup_all().await;
            return Err(ApiError::Failure("failed to commit transaction".to_string()));
        }

        Self::audit_log(
            "upload_complete",
            &format!("Upload process completed successfully for {}/{}", product.name, version.name),
            Some(&product.name),
            Some(&version.name),
        );

        Ok(Json(SymbolsResponse {
            result: "ok".to_string(),
        }))
    }
}
