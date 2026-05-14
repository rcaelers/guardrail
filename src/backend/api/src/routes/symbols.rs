use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{Json, http::HeaderMap};
use object_store::ObjectStoreExt;
use object_store::path::Path;
use serde::Deserialize;
use serde::Serialize;
use tracing::{error, info, instrument};

use crate::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product_by_id, get_product_by_ingestion_token, validate_api_token_for_product};
use crate::utils::{peek_line, stream_to_s3};
use data::api_token::ApiToken;
use data::product::Product;

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
    product: Option<String>,
    annotations: std::collections::HashMap<String, String>,
    symbols: Option<Symbols>,
}

#[derive(Default, Debug, Serialize)]
struct SymbolsContext {
    product_id: String,
    version: String,
    channel: String,
    commit: String,
    build_id: String,
}

#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    pub result: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct ApiKeyQuery {
    pub api_key: Option<String>,
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

    #[instrument(skip(product, api_token, symbols_info))]
    async fn validate_symbols(
        product: &Product,
        api_token: Option<&ApiToken>,
        symbols_info: &mut SymbolsInfo,
    ) -> Result<SymbolsContext, ApiError> {
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

            match field_name {
                "product" => product_name = value,
                "version" => version = value,
                "channel" => channel = value,
                "commit" => commit = value,
                "build_id" => build_id = value,
                _ => {}
            }
        }

        let name_matches = product_name.eq_ignore_ascii_case(&product.name)
            || product_name.eq_ignore_ascii_case(&product.slug);
        if !name_matches {
            error!(
                submitted = %product_name,
                authenticated = %product.name,
                "Submitted product name does not match authenticated product"
            );
            return Err(ApiError::ProductAccessDenied(product_name));
        }

        if let Some(token) = api_token {
            validate_api_token_for_product(token, product, &product_name)?;
        }

        if !product.accepting_crashes {
            return Err(ApiError::ProductNotAcceptingCrashes(product_name));
        }

        Ok(SymbolsContext {
            product_id: product.id.clone(),
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

    #[instrument(skip(symbols_info, symbols_context))]
    fn build_symbol_info(
        symbols_info: &SymbolsInfo,
        symbols_context: &SymbolsContext,
    ) -> Result<serde_json::Value, ApiError> {
        let symbols = symbols_info.symbols.as_ref().ok_or_else(|| {
            error!("No symbols found to queue");
            ApiError::Failure("no symbols found to queue".to_string())
        })?;

        let symbol_upload_id = uuid::Uuid::new_v4();

        Ok(serde_json::json!({
            "symbol_upload_id": symbol_upload_id.to_string(),
            "product_id": symbols_context.product_id,
            "os": symbols.header.os,
            "arch": symbols.header.arch,
            "build_id": symbols.header.build_id,
            "module_id": symbols.header.module_id,
            "storage_path": symbols.storage_path,
            "filename": symbols.filename,
            "submission_timestamp": symbols_info.submission_timestamp,
        }))
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

    #[instrument(skip(state, api_token, product, multipart))]
    async fn handle_upload(
        state: AppState,
        api_token: Option<ApiToken>,
        product: Product,
        mut multipart: Multipart,
        symbols_info: &mut SymbolsInfo,
    ) -> Result<(), ApiError> {
        symbols_info.product = Some(product.name.clone());

        info!(product = %product.name, "Processing symbol for product");

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!(error = ?e, "Failed to get next multipart field");
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, symbols_info, state.clone()).await?;
        }

        let symbol_context = Self::validate_symbols(&product, api_token.as_ref(), symbols_info).await?;

        let symbol_info_json = Self::build_symbol_info(symbols_info, &symbol_context)?;

        state
            .worker
            .queue_symbol(symbol_info_json)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to queue symbol job");
                ApiError::Failure("failed to queue symbol job".to_string())
            })?;

        Ok(())
    }

    #[instrument(skip(state, headers, api_key_query, multipart), fields(crash_id))]
    pub async fn upload(
        State(state): State<AppState>,
        headers: HeaderMap,
        api_key_query: Query<ApiKeyQuery>,
        multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        let db = &state.repo.db;

        let (api_token, product) = match crate::access::require_entitlement(
            &headers,
            api_key_query.api_key.as_deref(),
            db,
            "symbol-upload",
        )
        .await
        {
            Ok(token) => {
                let product_id = token.product_id.as_deref().ok_or_else(|| {
                    ApiError::ProductAccessDenied(
                        "API token is not associated with any product".to_string(),
                    )
                })?;
                let product = get_product_by_id(db, product_id).await?;
                (Some(token), product)
            }
            Err(_) => {
                let token_str = crate::access::extract_bearer_from_headers(&headers)
                    .ok_or_else(|| ApiError::InvalidToken("missing token".into()))?;
                let product = get_product_by_ingestion_token(db, token_str)
                    .await?
                    .ok_or_else(|| ApiError::InvalidToken("invalid token".into()))?;
                (None, product)
            }
        };

        let mut symbols_info = SymbolsInfo {
            submission_timestamp: chrono::Utc::now().to_rfc3339(),
            product: None,
            annotations: std::collections::HashMap::new(),
            ..Default::default()
        };

        let r = Self::handle_upload(state.clone(), api_token, product, multipart, &mut symbols_info).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::{Request, header::CONTENT_TYPE};
    use object_store::memory::InMemory;
    use repos::Repo;
    use std::sync::Arc;

    use crate::worker::TestWorker;

    fn symbols_info_with_symbols() -> SymbolsInfo {
        SymbolsInfo {
            submission_timestamp: "2024-01-01T00:00:00Z".to_string(),
            product: Some("TestProduct".to_string()),
            annotations: std::collections::HashMap::new(),
            symbols: Some(Symbols {
                filename: "app.sym".to_string(),
                size: 12,
                storage_path: "symbols/app.pdb-BUILD".to_string(),
                storage_filename: "app.pdb-BUILD".to_string(),
                header: SymbolsHeader {
                    os: "Linux".to_string(),
                    arch: "x86_64".to_string(),
                    build_id: "AABBCC".to_string(),
                    module_id: "app.pdb".to_string(),
                },
            }),
        }
    }

    async fn state() -> AppState {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        AppState {
            repo: Repo::new(db),
            settings: Arc::new(common::settings::Settings::default()),
            storage: Arc::new(InMemory::new()),
            worker: Arc::new(TestWorker::new()),
        }
    }

    #[test]
    fn validates_build_and_module_identifiers() {
        assert!(SymbolsApi::validate_build_id("ABCDEF-1234").is_ok());
        assert!(matches!(
            SymbolsApi::validate_build_id(""),
            Err(ApiError::Failure(message)) if message == "invalid build_id length"
        ));
        assert!(matches!(
            SymbolsApi::validate_build_id(&"a".repeat(65)),
            Err(ApiError::Failure(message)) if message == "invalid build_id length"
        ));
        assert!(matches!(
            SymbolsApi::validate_build_id("not hex"),
            Err(ApiError::Failure(message)) if message == "invalid build_id format"
        ));

        assert!(SymbolsApi::validate_module_id("app.pdb").is_ok());
        assert!(matches!(
            SymbolsApi::validate_module_id(""),
            Err(ApiError::Failure(message)) if message == "invalid module_id length"
        ));
        assert!(matches!(
            SymbolsApi::validate_module_id(&"a".repeat(257)),
            Err(ApiError::Failure(message)) if message == "invalid module_id length"
        ));
        for bad in ["../app.pdb", "dir/app.pdb", "dir\\app.pdb", "bad&pdb"] {
            assert!(matches!(
                SymbolsApi::validate_module_id(bad),
                Err(ApiError::Failure(message)) if message == "invalid module_id format"
            ));
        }
    }

    #[tokio::test]
    async fn processes_symbols_header() {
        let header = SymbolsApi::process_header("MODULE Linux x86_64 AABBCC app.pdb\n".to_string())
            .await
            .unwrap();
        assert_eq!(header.os, "Linux");
        assert_eq!(header.arch, "x86_64");
        assert_eq!(header.build_id, "AABBCC");
        assert_eq!(header.module_id, "app.pdb");

        assert!(matches!(
            SymbolsApi::process_header("MODULE Linux AABBCC app.pdb\n".to_string()).await,
            Err(ApiError::Failure(message)) if message == "invalid symbols file header"
        ));
    }

    #[test]
    fn validates_annotation_and_symbols_content_types() {
        assert!(SymbolsApi::validate_annotation_content_type("text/plain").is_ok());
        assert!(SymbolsApi::validate_annotation_content_type("text/markdown").is_ok());
        assert!(SymbolsApi::validate_annotation_content_type("").is_ok());
        assert!(matches!(
            SymbolsApi::validate_annotation_content_type("application/json"),
            Err(ApiError::Failure(message))
                if message == "invalid annotation content type: application/json"
        ));

        assert!(SymbolsApi::validate_symbols_content_type("application/octet-stream").is_ok());
        assert!(matches!(
            SymbolsApi::validate_symbols_content_type("text/plain"),
            Err(ApiError::Failure(message)) if message == "invalid symbols content type: text/plain"
        ));
    }

    #[test]
    fn validates_annotation_keys() {
        assert!(SymbolsApi::validate_key("product").is_ok());
        assert!(matches!(
            SymbolsApi::validate_key("bad\nkey"),
            Err(ApiError::Failure(message))
                if message == "annotation key must contain only printable ASCII characters"
        ));
        assert!(matches!(
            SymbolsApi::validate_key("caf\u{00e9}"),
            Err(ApiError::Failure(message))
                if message == "annotation key must contain only printable ASCII characters"
        ));
    }

    #[test]
    fn build_symbol_info_requires_symbols_and_uses_header_metadata() {
        let context = SymbolsContext {
            product_id: "products:one".to_string(),
            version: "1.0".to_string(),
            channel: "stable".to_string(),
            commit: "abc".to_string(),
            build_id: "AABBCC".to_string(),
        };

        assert!(matches!(
            SymbolsApi::build_symbol_info(&SymbolsInfo::default(), &context),
            Err(ApiError::Failure(message)) if message == "no symbols found to queue"
        ));

        let info = symbols_info_with_symbols();
        let value = SymbolsApi::build_symbol_info(&info, &context).unwrap();
        assert!(value["symbol_upload_id"].as_str().is_some());
        assert_eq!(value["product_id"], "products:one");
        assert_eq!(value["os"], "Linux");
        assert_eq!(value["arch"], "x86_64");
        assert_eq!(value["build_id"], "AABBCC");
        assert_eq!(value["module_id"], "app.pdb");
        assert_eq!(value["storage_path"], "symbols/app.pdb-BUILD");
        assert_eq!(value["filename"], "app.sym");
        assert_eq!(value["submission_timestamp"], "2024-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn handle_symbols_upload_rejects_duplicate_symbols_before_reading_field() {
        let mut info = symbols_info_with_symbols();
        let boundary = "----guardrail-api-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"app.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE Linux x86 AABBCC app.pdb\n\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let test_state = state().await;
        let mut multipart = Multipart::from_request(request, &test_state).await.unwrap();
        let field = multipart.next_field().await.unwrap().unwrap();

        assert!(matches!(
            SymbolsApi::handle_symbols_upload(field, &mut info, test_state).await,
            Err(ApiError::Failure(message)) if message == "symbols file already processed"
        ));
    }

    #[tokio::test]
    async fn handle_annotation_upload_requires_field_name() {
        let boundary = "----guardrail-api-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data\r\nContent-Type: text/plain\r\n\r\nvalue\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let test_state = state().await;
        let mut multipart = Multipart::from_request(request, &test_state).await.unwrap();
        let field = multipart.next_field().await.unwrap().unwrap();
        let mut info = SymbolsInfo::default();

        assert!(matches!(
            SymbolsApi::handle_annotation_upload(field, &mut info).await,
            Err(ApiError::Failure(message)) if message == "name field is missing for annotation"
        ));
    }
}
