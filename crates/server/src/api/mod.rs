pub mod error;
use std::path::PathBuf;

use repos::{
    api_token::ApiToken, product::{Product, ProductRepo}, version::{Version, VersionRepo}
};
pub use routes::routes;

mod api_token;
mod file_cleanup;
mod minidump;
mod routes;
mod symbols;
mod token;
mod webauthn;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::BoxError;
use axum::body::Bytes;
use futures::prelude::*;
use rand::{Rng, distr::Alphanumeric, rng};
use sqlx::Postgres;
use tokio::fs::File;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;
use tracing::error;

use error::ApiError;

pub fn hash_token(token: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(token.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

pub fn verify_token(token: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();

    match argon2.verify_password(token.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e),
    }
}

#[allow(dead_code)]
pub fn generate_token() -> String {
    let mut rng = rng();
    let token: String = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(36)
        .collect();
    token
}

async fn stream_to_file<S, E>(path: &std::path::PathBuf, stream: S) -> Result<(), ApiError>
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

async fn get_product<E>(tx: &mut E, product_name: &str) -> Result<Product, ApiError>
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

async fn get_version<E>(
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

fn validate_api_token_for_product(
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

async fn validate_file_size(
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
        error!(
            "{} too large: {} bytes (max: {} bytes)",
            file_type,
            file_metadata.len(),
            max_size
        );
        return Err(ApiError::Failure(format!("{} file too large", file_type)));
    }

    if file_metadata.len() == 0 {
        let _ = tokio::fs::remove_file(file_path).await;
        error!("Empty {} file", file_type);
        return Err(ApiError::Failure(format!("empty {} file", file_type)));
    }

    Ok(file_metadata.len())
}

