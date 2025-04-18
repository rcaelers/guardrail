use axum::BoxError;
use axum::body::Bytes;
use axum::extract::multipart::Field;
use futures::{Stream, StreamExt, TryStreamExt};
use object_store::{ObjectStore, path::Path};
use sqlx::Postgres;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::io::{self};
use tokio_util::io::StreamReader;
use tracing::error;

use crate::error::ApiError;
use data::{api_token::ApiToken, product::Product, version::Version};
use repos::{product::ProductRepo, version::VersionRepo};

pub async fn peek_line<'a>(
    field: Field<'a>,
) -> io::Result<(String, impl Stream<Item = Result<Bytes, BoxError>> + Send + 'a)> {
    let mut stream = field.map_err(io::Error::other);

    let mut buffer = Vec::new();
    let mut first_line = None;
    let mut remaining_buffer = None;

    while first_line.is_none() {
        if let Some(bytes_result) = stream.next().await {
            let bytes = bytes_result?;
            buffer.extend_from_slice(&bytes);

            if let Some(newline_pos) = buffer.iter().position(|&b| b == b'\n') {
                let line = String::from_utf8_lossy(&buffer[..=newline_pos]).to_string();
                first_line = Some(line);

                if newline_pos < buffer.len() - 1 {
                    remaining_buffer = Some(Bytes::copy_from_slice(&buffer[(newline_pos + 1)..]));
                }
            }
        } else {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "No complete line found"));
        }
    }

    let line = first_line.unwrap();

    let mut initial_chunks: Vec<Result<Bytes, BoxError>> =
        vec![Ok(Bytes::copy_from_slice(line.as_bytes()))];

    if let Some(remaining) = remaining_buffer {
        initial_chunks.push(Ok(remaining));
    }

    let combined_stream = futures::stream::iter(initial_chunks)
        .chain(stream.map(|result| result.map_err(BoxError::from)));

    Ok((line, combined_stream))
}

pub async fn stream_to_s3<S, E>(
    store: Arc<dyn ObjectStore>,
    key: &str,
    stream: S,
) -> Result<(), ApiError>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        let body_with_io_error = stream.map_err(io::Error::other);
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);
        let path = Path::from(key);

        let mut writer = object_store::buffered::BufWriter::new(store, path);

        tokio::io::copy(&mut body_reader, &mut writer)
            .await
            .map_err(|e| {
                error!("Failed to copy stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        writer.shutdown().await.map_err(|e| {
            error!("Failed to shutdown buffered writer: {:?}", e);
            ApiError::InternalFailure()
        })?;

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
            return Err(ApiError::ProductAccessDenied(product_name.to_owned()));
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
