use super::error::ApiError;
use crate::api::stream_to_file;
use crate::app_state::AppState;
use crate::settings;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::Json;
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
        tokio::fs::create_dir_all(&upload_path).await?;
        Ok(symbol_file)
    }

    async fn get_header(symbol_file: &PathBuf) -> Result<String, ApiError> {
        let file = File::open(symbol_file).await.expect("Failed to open file");
        info!("open");
        let mut reader = BufReader::new(file);
        let mut first_line = String::new();
        reader.read_line(&mut first_line).await?;

        Ok(first_line)
    }

    async fn process_symbol_file(symbol_file: &PathBuf) -> Result<SymbolsData, ApiError> {
        let first_line = Self::get_header(symbol_file).await?;

        let collection: Vec<&str> = first_line.split_whitespace().collect();
        let os = String::from(collection[1]);
        let arch = String::from(collection[2]);
        let build_id = String::from(collection[3]);
        let module_id = String::from(collection[4]);

        let final_path = std::path::Path::new(&settings().server.base_path)
            .join("symbols")
            .join(&module_id)
            .join(&build_id);
        tokio::fs::create_dir_all(&final_path).await?;
        let final_file = final_path.join(module_id.replace(".pdb", ".sym"));

        let r = SymbolsData {
            os,
            arch,
            build_id,
            module_id,
            file_location: final_file.to_str().unwrap_or("").to_string(),
        };

        fs::rename(&symbol_file, &final_file).await?;
        Ok(r)
    }

    async fn store(
        tx: impl sqlx::Executor<'_, Database = Postgres>,
        data: SymbolsData,
        product: repos::product::Product,
        version: repos::version::Version,
    ) -> Result<(), ApiError> {
        let dto = NewSymbols {
            os: data.os,
            arch: data.arch,
            build_id: data.build_id,
            module_id: data.module_id,
            file_location: data.file_location,
            product_id: product.id,
            version_id: version.id,
        };
        SymbolsRepo::create(tx, dto)
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("error: {:?}", e);
                ApiError::Failure
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
            error!("error: {:?}", e);
            ApiError::Failure
        })?;

        let symbol_file = Self::get_temp_symbols_file().await?;

        let product = ProductRepo::get_by_name(&mut *tx, &params.product)
            .await?
            .ok_or(ApiError::Failure)?;
        info!("product: {:?}", product);

        let version = VersionRepo::get_by_product_and_name(&mut *tx, product.id, &params.version)
            .await?
            .ok_or(ApiError::Failure)?;
        info!("version : {:?}", version);

        stream_to_file(&symbol_file, field).await?;
        info!("received symbol file: {:?}", symbol_file);

        let data = Self::process_symbol_file(&symbol_file).await?;
        info!(
            "processed symbol file: {:?} {:?}",
            symbol_file, data.build_id
        );

        Self::store(&mut *tx, data, product, version).await?;
        info!("stored symbol file: {:?}", symbol_file);

        tx.commit().await.map_err(|e| {
            error!("error: {:?}", e);
            ApiError::Failure
        })?;
        Ok(())
    }

    pub async fn upload(
        State(state): State<AppState>,
        Query(params): Query<SymbolsRequestParams>,
        //JwtClaims(user): JwtClaims<User>,
        mut multipart: Multipart,
    ) -> Result<Json<SymbolsResponse>, ApiError> {
        //info!("user: {:?}", user);
        while let Some(field) = multipart.next_field().await? {
            match field.name() {
                Some("upload_file_symbols") => {
                    Self::handle_symbol_upload(&state, &params, field).await?
                }
                Some("options") => {
                    let content = field.bytes().await?;
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
