use axum::body::Bytes;
use axum::BoxError;
use futures::prelude::*;
use tokio::fs::File;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;

use super::error::UtilsError;

pub async fn stream_to_file<S, E>(path: &std::path::PathBuf, stream: S) -> Result<(), UtilsError>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        let mut file = BufWriter::new(File::create(path).await?);
        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<(), UtilsError>(())
    }
    .await
    .map_err(|_err| (UtilsError::Failure))
}
