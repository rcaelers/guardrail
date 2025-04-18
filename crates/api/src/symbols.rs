use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, get_version, validate_api_token_for_product};
use crate::utils::{peek_line, stream_to_s3};
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use axum_extra::extract::WithRejection;
use data::api_token::ApiToken;
use data::product::Product;
use data::symbols::NewSymbols;
use data::version::Version;
use repos::symbols::SymbolsRepo;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use tracing::{debug, error, info};

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

    async fn process_header(
        first_line: String,
        product: data::product::Product,
        version: data::version::Version,
    ) -> Result<NewSymbols, ApiError> {
        let collection: Vec<&str> = first_line.split_whitespace().collect();
        if collection.len() < 5 {
            error!("invalid symbols file header: {:?}", first_line);
            return Err(ApiError::Failure("invalid symbols file header".to_string()));
        }
        let os = String::from(collection[1]);
        let arch = String::from(collection[2]);
        let build_id = String::from(collection[3]);
        let module_id = String::from(collection[4]);
        let path = format!("symbols/{}-{}", module_id, build_id);

        Self::validate_build_id(&build_id)?;
        Self::validate_module_id(&module_id)?;

        Ok(NewSymbols {
            os,
            arch,
            build_id,
            module_id,
            file_location: path,
            product_id: product.id,
            version_id: version.id,
        })
    }

    async fn handle_symbol_upload<E>(
        tx: &mut E,
        product: &Product,
        version: &Version,
        field: Field<'_>,
        state: AppState,
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

        let (line, stream) = peek_line(field).await.map_err(|e| {
            error!("failed to read symbols file: {:?}", e);
            ApiError::Failure("failed to read symbols file".to_string())
        })?;

        let data = Self::process_header(line, product.clone(), version.clone()).await?;

        SymbolsRepo::create(tx, data.clone()).await.map_err(|e| {
            error!("failed to stored symbols {:?}", e);
            ApiError::InternalFailure()
        })?;

        if let Err(e) = stream_to_s3(state.storage.clone(), &data.file_location, stream).await {
            error!("Failed to stream to S3: {:?}", e);
            return Err(ApiError::InternalFailure());
        }

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
        state: AppState,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        match field.name() {
            Some("upload_file_symbols") => {
                Self::handle_symbol_upload(&mut *tx, product, version, field, state).await?;
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
        WithRejection(Query(params), _): WithRejection<Query<SymbolsRequestParams>, ApiError>,
        mut multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        debug!("SymbolsApi::upload called with params: {:?}", params);
        tracing::info!("Logging initialized");

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

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("Failed to get next multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(&mut *tx, field, &product, &version, state.clone()).await?;
        }

        let commit_result = tx.commit().await;
        if let Err(e) = commit_result {
            error!("Failed to commit transaction: {:?}", e);
            return Err(ApiError::Failure("failed to commit transaction".to_string()));
        }

        //TODO: remove from storage if commit fails

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
