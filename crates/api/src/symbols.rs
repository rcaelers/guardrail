use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, validate_api_token_for_product};
use crate::utils::{peek_line, stream_to_s3};
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Extension, Json};
use axum_extra::extract::WithRejection;
use data::api_token::ApiToken;
use data::product::Product;
use data::symbols::NewSymbols;
use repos::symbols::SymbolsRepo;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
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

pub struct SymbolsApi;

impl SymbolsApi {
    fn validate_symbols_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "application/octet-stream";
        if !is_valid {
            error!("Invalid symbols content type: {}", content_type);
            return Err(ApiError::Failure(format!("invalid symbols content type: {content_type}")));
        }
        Ok(())
    }

    fn validate_build_id(build_id: &str) -> Result<(), ApiError> {
        if build_id.is_empty() || build_id.len() > 64 {
            error!("Invalid build_id length: {}", build_id);
            return Err(ApiError::Failure("invalid build_id length".to_string()));
        }

        if !build_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            error!("Invalid build_id format: {}", build_id);
            return Err(ApiError::Failure("invalid build_id format".to_string()));
        }

        Ok(())
    }

    fn validate_module_id(module_id: &str) -> Result<(), ApiError> {
        if module_id.is_empty() || module_id.len() > 256 {
            error!("Invalid module_id length: {}", module_id);
            return Err(ApiError::Failure("invalid module_id length".to_string()));
        }

        if module_id.contains("..") || module_id.contains('/') || module_id.contains('\\') {
            error!("Invalid module_id contains path traversal characters: {}", module_id);
            return Err(ApiError::Failure("invalid module_id format".to_string()));
        }

        if !module_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        {
            error!("Invalid module_id format: {}", module_id);
            return Err(ApiError::Failure("invalid module_id format".to_string()));
        }

        Ok(())
    }

    async fn process_header(
        first_line: String,
        product: data::product::Product,
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
        let path = format!("symbols/{module_id}-{build_id}");

        Self::validate_build_id(&build_id)?;
        Self::validate_module_id(&module_id)?;

        Ok(NewSymbols {
            os,
            arch,
            build_id,
            module_id,
            storage_location: path,
            product_id: product.id,
        })
    }

    async fn handle_symbol_upload<E>(
        tx: &mut E,
        product: &Product,
        field: Field<'_>,
        state: AppState,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        let content_type = field.content_type().unwrap_or_default();
        Self::validate_symbols_content_type(content_type)?;

        let (line, stream) = peek_line(field).await.map_err(|e| {
            error!("failed to read symbols file: {:?}", e);
            ApiError::Failure("failed to read symbols file".to_string())
        })?;

        let data = Self::process_header(line, product.clone()).await?;

        SymbolsRepo::create(tx, data.clone()).await?;

        if let Err(e) = stream_to_s3(state.storage.clone(), &data.storage_location, stream).await {
            error!("Failed to stream to S3: {:?}", e);
            return Err(ApiError::InternalFailure());
        }

        Ok(())
    }

    async fn process_field<E>(
        tx: &mut E,
        field: Field<'_>,
        product: &Product,
        state: AppState,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        match field.name() {
            Some("symbols_file") => {
                Self::handle_symbol_upload(&mut *tx, product, field, state).await?;
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
        params.validate()?;

        info!("Starting symbols upload process for {} {}", params.product, params.version);

        let mut tx = state.repo.begin_admin().await?;

        let product = get_product(&mut *tx, &params.product).await?;
        validate_api_token_for_product(&api_token, &product, &params.product)?;

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("Failed to get next multipart field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(&mut *tx, field, &product, state.clone()).await?;
        }
        state.repo.end(tx).await?;

        info!("Upload process completed successfully for {} {}", params.product, params.version);

        Ok(Json(SymbolsResponse {
            result: "ok".to_string(),
        }))
    }
}
