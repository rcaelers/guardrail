use super::error::ApiError;
use crate::api::stream_to_file;
use crate::app_state::AppState;
use crate::settings;
use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use repos::product::ProductRepo;
use repos::symbols::{NewSymbols, SymbolsRepo};
use repos::version::VersionRepo;
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

#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    pub result: String,
}

#[derive(Debug, Serialize)]
struct SymbolsData {
    pub os: String,
    pub arch: String,
    pub build_id: String,
    pub module_id: String,
    pub file_location: String,
}

pub struct SymbolsApi;

impl SymbolsApi {
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

    async fn process_symbol_file(symbol_file: &PathBuf) -> Result<SymbolsData, ApiError> {
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

    async fn handle_symbol_upload(
        state: &AppState,
        params: &SymbolsRequestParams,
        field: Field<'_>,
    ) -> Result<(), ApiError> {
        info!("handle_symbol_upload");
        let mut tx = state.repo.begin_admin().await.map_err(|e| {
            error!("failed to start transaction: {:?}", e);
            ApiError::InternalFailure()
        })?;

        let symbol_file = Self::get_temp_symbols_file().await?;

        let product = ProductRepo::get_by_name(&mut *tx, &params.product)
            .await
            .map_err(|e| {
                error!("failed to get product {:?}: {:?}", params.product, e);
                ApiError::ProductNotFound(params.product.clone())
            })?
            .ok_or_else(|| {
                error!("product not found: {:?}", params.product);
                ApiError::ProductNotFound(params.product.clone())
            })?;
        info!("product: {:?}", product);

        let version = VersionRepo::get_by_product_and_name(&mut *tx, product.id, &params.version)
            .await
            .map_err(|e| {
                error!("failed to get version {:?}: {:?}", params.version, e);
                ApiError::VersionNotFound(params.product.clone(), params.version.clone())
            })?
            .ok_or_else(|| {
                error!("version not found: {:?}", params.version);
                ApiError::VersionNotFound(params.product.clone(), params.version.clone())
            })?;
        info!("version : {:?}", version);

        stream_to_file(&symbol_file, field).await.map_err(|e| {
            error!("failed to save symbols file {:?}: {:?}", symbol_file, e);
            ApiError::InternalFailure()
        })?;
        info!("received symbol file: {:?}", symbol_file);

        let data = Self::process_symbol_file(&symbol_file).await?;
        info!("processed symbol file: {:?} {:?}", symbol_file, data.build_id);

        Self::store(&mut *tx, data, product, version).await?;
        info!("stored symbol file: {:?}", symbol_file);

        tx.commit().await.map_err(|e| {
            error!("failed to commit transaction: {:?}", e);
            ApiError::InternalFailure()
        })?;
        Ok(())
    }

    pub async fn upload(
        State(state): State<AppState>,
        Query(params): Query<SymbolsRequestParams>,
        mut multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!("failed to read field: {:?}", e);
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            match field.name() {
                Some("upload_file_symbols") => {
                    Self::handle_symbol_upload(&state, &params, field).await?
                }
                Some("options") => {
                    let content = field.bytes().await.map_err(|e| {
                        error!("failed to read options field: {:?}", e);
                        ApiError::Failure("failed to read options field".to_string())
                    })?;
                    info!("options: {:?}", content);
                }
                _ => (),
            }
        }
        Ok(Json(SymbolsResponse {
            result: "ok".to_string(),
        }))
    }
}
