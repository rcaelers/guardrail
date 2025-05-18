use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product, get_product_by_id, validate_api_token_for_product};
use crate::utils::{peek_line, stream_to_s3};
use axum::extract::multipart::Field;
use axum::extract::{Multipart, State};
use axum::{Extension, Json};
use data::api_token::ApiToken;
use data::symbols::NewSymbols;
use object_store::path::Path;
use repos::symbols::SymbolsRepo;
use serde::Serialize;
use sqlx::Postgres;
use tracing::{error, info, instrument};

#[derive(Default, Debug, Serialize)]
struct SymbolsHeader {
    os: String,
    arch: String,
    build_id: String,
    module_id: String,
}

#[derive(Default, Debug, Serialize)]
struct Symbols {
    filename: String,
    size: u64,
    storage_path: String,
    storage_filename: String,
    header: SymbolsHeader,
}

#[derive(Default, Debug, Serialize)]
struct SymbolsInfo {
    submission_timestamp: String,
    authorized_product: Option<String>,
    annotations: std::collections::HashMap<String, String>,
    symbols: Option<Symbols>,
}

#[derive(Default, Debug, Serialize)]
struct SymbolsContext {
    product_id: uuid::Uuid,
    version: String,
    channel: String,
    commit: String,
    build_id: String,
}

#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    pub result: String,
}

pub struct SymbolsApi;

impl SymbolsApi {
    const REQUIRED_FIELDS: &'static [&'static str] =
        &["product", "version", "channel", "commit", "build_id"];

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

    #[instrument(skip(tx, api_token, symbols_info))]
    async fn validate_symbols<E>(
        tx: &mut E,
        api_token: &ApiToken,
        symbols_info: &mut SymbolsInfo,
    ) -> Result<SymbolsContext, ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        if symbols_info.symbols.is_none() {
            error!("No symbols found in submission");
            return Err(ApiError::Failure("no symbols found in submission".to_string()));
        }

        let mut product_name = String::new();
        let mut version = String::new();
        let mut channel = String::new();
        let mut commit = String::new();
        let mut build_id = String::new();

        for &field_name in Self::REQUIRED_FIELDS {
            let value = symbols_info.annotations.remove(field_name);

            let value = match value {
                Some(v) if !v.trim().is_empty() => v,
                Some(_) => {
                    error!("Required annotation '{}' is empty", field_name);
                    return Err(ApiError::Failure(format!(
                        "required annotation '{field_name}' cannot be empty"
                    )));
                }
                None => {
                    error!("Required annotation '{}' is missing", field_name);
                    return Err(ApiError::Failure(format!(
                        "required annotation '{field_name}' is missing"
                    )));
                }
            };

            // Assign to the appropriate variable
            match field_name {
                "product" => product_name = value,
                "version" => version = value,
                "channel" => channel = value,
                "commit" => commit = value,
                "build_id" => build_id = value,
                _ => {}
            }
        }

        let product = get_product(tx, &product_name).await?;
        validate_api_token_for_product(api_token, &product, &product_name)?;
        if !product.accepting_crashes {
            return Err(ApiError::ProductNotAcceptingCrashes(product_name));
        }

        Ok(SymbolsContext {
            product_id: product.id,
            version,
            channel,
            commit,
            build_id,
        })
    }

    async fn process_header(first_line: String) -> Result<SymbolsHeader, ApiError> {
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

        Ok(SymbolsHeader {
            os,
            arch,
            build_id,
            module_id,
        })
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

    fn validate_symbols_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "application/octet-stream";

        if !is_valid {
            error!(content_type, "Invalid symbols content type");
            return Err(ApiError::Failure(format!("invalid symbols content type: {content_type}")));
        }
        Ok(())
    }

    #[instrument(skip(tx, symbols_info, symbols_context))]
    async fn store_symbols<E>(
        tx: &mut E,
        symbols_info: &SymbolsInfo,
        symbols_context: &SymbolsContext,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        if let Some(symbols) = &symbols_info.symbols {
            info!(storage_path = %symbols.storage_path, "Storing symbols in database");

            let new_symbols = NewSymbols {
                os: symbols.header.os.clone(),
                arch: symbols.header.arch.clone(),
                build_id: symbols.header.build_id.clone(),
                module_id: symbols.header.module_id.clone(),
                product_id: symbols_context.product_id,
                storage_path: symbols.storage_path.clone(),
            };

            let result = SymbolsRepo::create(tx, new_symbols).await?;
            info!(symbol_id = %result, "Symbols stored in database");
        } else {
            error!("No symbols found to store");
            return Err(ApiError::Failure("no symbols found to store".to_string()));
        }
        Ok(())
    }

    #[instrument(skip(field, symbols_info))]
    async fn handle_annotation_upload(
        field: Field<'_>,
        symbols_info: &mut SymbolsInfo,
    ) -> Result<(), ApiError> {
        info!("Processing symbols annotation");
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

        symbols_info.annotations.insert(key, value);
        Ok(())
    }

    #[instrument(skip(field, symbols_info, state))]
    async fn handle_symbols_upload(
        field: Field<'_>,
        symbols_info: &mut SymbolsInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        info!("Processing symbols");

        if symbols_info.symbols.is_some() {
            error!("Symbols file already processed");
            return Err(ApiError::Failure("symbols file already processed".to_string()));
        }

        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_symbols_content_type(&content_type)?;

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_symbols".to_string());

        let (line, stream) = peek_line(field).await.map_err(|e| {
            error!("failed to read symbols file: {:?}", e);
            ApiError::Failure("failed to read symbols file".to_string())
        })?;

        let header = Self::process_header(line).await?;

        let storage_filename = format!("{}-{}", header.module_id, header.build_id);
        let storage_path = format!("symbols/{storage_filename}");

        let size = stream_to_s3(state.storage.clone(), &storage_path, stream)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to stream symbols to S3");
                ApiError::Failure("failed to store symbols".to_string())
            })?;

        symbols_info.symbols = Some(Symbols {
            filename,
            size,
            storage_path,
            storage_filename,
            header,
        });

        Ok(())
    }

    #[instrument(skip(field, symbols_info, state))]
    async fn process_field(
        field: Field<'_>,
        symbols_info: &mut SymbolsInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        let field_name = field.name().unwrap_or_default();

        match field_name {
            "upload_file_symbols" => Self::handle_symbols_upload(field, symbols_info, state).await,
            _ => Self::handle_annotation_upload(field, symbols_info).await,
        }
    }

    #[instrument(skip(state, api_token, multipart))]
    pub async fn handle_upload(
        state: AppState,
        api_token: ApiToken,
        mut multipart: Multipart,
        symbols_info: &mut SymbolsInfo,
    ) -> Result<(), ApiError> {
        let mut tx = state.repo.begin_admin().await?;

        let product_id = api_token.product_id.ok_or_else(|| {
            error!("API token does not have a product ID");
            ApiError::ProductAccessDenied(
                "API token is not associated with any product".to_string(),
            )
        })?;

        let authorized_product = get_product_by_id(&mut *tx, product_id).await?;
        symbols_info.authorized_product = Some(authorized_product.name.clone());

        info!(product = %authorized_product.name, "Processing symbol for product");

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!(error = ?e, "Failed to get next multipart field");
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, symbols_info, state.clone()).await?;
        }

        let symbol_context = Self::validate_symbols(&mut *tx, &api_token, symbols_info).await?;

        Self::store_symbols(&mut *tx, symbols_info, &symbol_context).await?;

        state.repo.end(tx).await?;

        Ok(())
    }

    #[instrument(skip(state, api_token, multipart), fields(crash_id))]
    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        let mut symbols_info = SymbolsInfo {
            submission_timestamp: chrono::Utc::now().to_rfc3339(),
            authorized_product: None,
            annotations: std::collections::HashMap::new(),
            ..Default::default()
        };

        let r = Self::handle_upload(state.clone(), api_token, multipart, &mut symbols_info).await;
        if let Err(e) = r {
            error!(error = ?e, "Failed to handle symbols upload");

            if let Some(symbol) = &symbols_info.symbols {
                info!(storage_path = %symbol.storage_path, "Deleting symbol from storage");
                let _ = state
                    .storage
                    .delete(&Path::from(symbol.storage_path.as_str()))
                    .await;
            }
            return Err(e);
        }

        Ok(Json(SymbolsResponse {
            result: "ok".to_string(),
        }))
    }
}
