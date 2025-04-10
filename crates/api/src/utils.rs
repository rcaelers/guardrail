use axum::BoxError;
use axum::body::Bytes;
use futures::prelude::*;
use sqlx::Postgres;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;
use tracing::error;

use crate::error::ApiError;
use data::{api_token::ApiToken, product::Product, version::Version};
use repos::{product::ProductRepo, version::VersionRepo};

pub async fn stream_to_file<S, E>(path: &std::path::PathBuf, stream: S) -> Result<(), ApiError>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        let body_with_io_error = stream.map_err(io::Error::other);
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        let file = File::create(path).await.map_err(|e| {
            error!("failed to create file {:?}: {:?}", path, e);
            ApiError::InternalFailure()
        })?;
        let mut file = BufWriter::new(file);
        let _r = tokio::io::copy(&mut body_reader, &mut file).await;

        Ok::<(), ApiError>(())
    }
    .await
    .map_err(|_err| (ApiError::InternalFailure()))
}

pub async fn get_product<E>(tx: &mut E, product_name: &str) -> Result<Product, ApiError>
where
    for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
{
    ProductRepo::get_by_name(tx, product_name)
        .await
        .map_err(|_| {
            error!("Failed to get product {}", product_name);
            ApiError::Failure(format!("failed to get product {}", product_name))
        })?
        .ok_or_else(|| {
            error!("No such product {}", product_name);
            ApiError::ProductNotFound(product_name.to_string())
        })
}

pub async fn get_version<E>(
    tx: &mut E,
    product: &Product,
    version_name: &str,
) -> Result<Version, ApiError>
where
    for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
{
    VersionRepo::get_by_product_and_name(tx, product.id, version_name)
        .await
        .map_err(|_| {
            error!("Failed to get version for {}/{}", product.name, version_name);
            ApiError::Failure(format!(
                "failed to get version for {}/{}",
                product.name, version_name
            ))
        })?
        .ok_or_else(|| {
            error!("No such version for {}/{}", product.name, version_name);
            ApiError::VersionNotFound(product.name.clone(), version_name.to_string())
        })
}

pub fn validate_api_token_for_product(
    api_token: &ApiToken,
    product: &Product,
    product_name: &str,
) -> Result<(), ApiError> {
    if let Some(token_product_id) = api_token.product_id {
        if token_product_id != product.id {
            error!(
                "API token not authorized for product {}, token is for product_id: {}",
                product_name, token_product_id
            );
            return Err(ApiError::Failure(format!(
                "API token not authorized for product {}",
                product_name
            )));
        }
    }
    Ok(())
}

pub async fn validate_file_size(
    file_path: &PathBuf,
    max_size: u64,
    file_type: &str,
) -> Result<u64, ApiError> {
    let file_metadata = tokio::fs::metadata(file_path).await.map_err(|e| {
        error!("Failed to get metadata for {} file: {:?}", file_type, e);
        ApiError::Failure(format!("failed to verify {} file", file_type))
    })?;

    if file_metadata.len() > max_size {
        let _ = tokio::fs::remove_file(file_path).await;
        error!("{} too large: {} bytes (max: {} bytes)", file_type, file_metadata.len(), max_size);
        return Err(ApiError::Failure(format!("{} file too large", file_type)));
    }

    if file_metadata.len() == 0 {
        let _ = tokio::fs::remove_file(file_path).await;
        error!("Empty {} file", file_type);
        return Err(ApiError::Failure(format!("empty {} file", file_type)));
    }

    Ok(file_metadata.len())
}
