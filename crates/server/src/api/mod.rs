pub mod error;
pub use routes::routes;

mod minidump;
mod routes;
mod symbols;

use axum::BoxError;
use axum::body::Bytes;
use futures::prelude::*;
use tokio::fs::File;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;

use error::ApiError;

// use rand::{distributions::Alphanumeric, thread_rng, Rng};

// pub fn make_api_key() -> String {
//     thread_rng()
//         .sample_iter(&Alphanumeric)
//         .take(64)
//         .map(char::from)
//         .collect()
// }

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
        let _r = tokio::io::copy(&mut body_reader, &mut file).await;

        Ok::<(), ApiError>(())
    }
    .await
    .map_err(|_err| (ApiError::Failure))
}
