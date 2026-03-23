use axum::BoxError;
use axum::body::Bytes;
use futures::{Stream, TryStreamExt};
use object_store::{ObjectStore, path::Path};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::io::{self};
use tokio_util::io::StreamReader;
use tracing::error;

use crate::error::ApiError;

pub async fn stream_to_s3<S, E>(
    store: Arc<dyn ObjectStore>,
    key: &str,
    stream: S,
) -> Result<u64, ApiError>
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

        let bytes_copied = tokio::io::copy(&mut body_reader, &mut writer)
            .await
            .map_err(|e| {
                error!("Failed to copy stream to S3: {:?}", e);
                ApiError::InternalFailure()
            })?;

        writer.shutdown().await.map_err(|e| {
            error!("Failed to shutdown buffered writer: {:?}", e);
            ApiError::InternalFailure()
        })?;

        Ok::<u64, ApiError>(bytes_copied)
    }
    .await
    .map_err(|_err| ApiError::InternalFailure())
}
