use super::base::NoneFilter;
use super::base::Resource;
use super::error::ApiError;
use crate::app_state::AppState;
use crate::model::base::Repo;
use crate::model::version::VersionRepo;
use crate::settings;
use crate::{
    entity::{prelude::Symbols, symbols},
    model::symbols::{SymbolsCreateDto, SymbolsUpdateDto},
};
use axum::body::Bytes;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::{BoxError, Json};
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, File};
use tokio::io::{self, AsyncBufReadExt, BufReader, BufWriter};
use tokio_util::io::StreamReader;
use tracing::{error, info};
use uuid::Uuid;

impl Resource for Symbols {
    type Entity = symbols::Entity;
    type ActiveModel = symbols::ActiveModel;
    type Data = symbols::Model;
    type CreateData = SymbolsCreateDto;
    type UpdateData = SymbolsUpdateDto;
    type Filter = NoneFilter;
}

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
            info!("start copy");
            let r = tokio::io::copy(&mut body_reader, &mut file).await;
            info!("r: {:?}", r);
            info!("end copy");

            Ok::<(), ApiError>(())
        }
        .await
        .map_err(|_err| (ApiError::Failure))
    }

    async fn get_product(
        state: &AppState,
        params: &SymbolsRequestParams,
    ) -> Result<crate::model::product::Product, ApiError> {
        let product = Repo::get_by_column::<crate::entity::product::Entity, _, _>(
            &state.db,
            crate::entity::product::Column::Name,
            params.product.clone(),
        )
        .await;
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
        state: &AppState,
        product_id: Uuid,
        params: &SymbolsRequestParams,
    ) -> Result<crate::model::version::Version, ApiError> {
        info!("get_version {:?} {:?}", product_id, params.version);
        let version =
            VersionRepo::get_by_product_and_name(&state.db, product_id, params.version.clone())
                .await;
        info!("get_version {:?}", version);
        let version = match version {
            Ok(version) => version,
            Err(e) => {
                error!("error: {:?}", e);
                return Err(ApiError::Failure);
            }
        }
        .ok_or(ApiError::Failure)?;
        info!("version: {:?}", version.id);
        Ok(version)
    }

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
        data: SymbolsData,
        product: crate::model::product::Product,
        version: crate::model::version::Version,
        state: &AppState,
    ) -> Result<(), ApiError> {
        let dto = SymbolsCreateDto {
            os: data.os,
            arch: data.arch,
            build_id: data.build_id,
            module_id: data.module_id,
            file_location: data.file_location,
            product_id: product.id,
            version_id: version.id,
        };
        Repo::create(&state.db, dto)
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
        let symbol_file = Self::get_temp_symbols_file().await?;

        let product = Self::get_product(state, params).await?;
        info!("product: {:?}", product);
        let version = Self::get_version(state, product.id, params).await?;
        info!("version : {:?}", version);

        Self::stream_to_file(&symbol_file, field).await?;
        info!("received symbol file: {:?}", symbol_file);

        let data = Self::process_symbol_file(&symbol_file).await?;
        info!(
            "processed symbol file: {:?} {:?}",
            symbol_file, data.build_id
        );

        Self::store(data, product, version, state).await?;
        info!("stored symbol file: {:?}", symbol_file);

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
