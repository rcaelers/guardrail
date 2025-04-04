pub mod error;
pub use routes::routes;

mod api_token;
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
