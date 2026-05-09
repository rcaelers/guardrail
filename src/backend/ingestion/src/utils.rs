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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn stream_to_s3_writes_all_chunks() {
        let store = Arc::new(InMemory::new());
        let chunks = stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"hello ")),
            Ok(Bytes::from_static(b"world")),
        ]);

        let written = stream_to_s3(store.clone(), "objects/test.txt", chunks)
            .await
            .unwrap();

        assert_eq!(written, 11);
        let bytes = store
            .get_opts(&Path::from("objects/test.txt"), Default::default())
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"hello world");
    }

    #[tokio::test]
    async fn stream_to_s3_maps_stream_errors() {
        let store = Arc::new(InMemory::new());
        let chunks = stream::iter([Err::<Bytes, _>(std::io::Error::other("boom"))]);

        let err = stream_to_s3(store, "objects/fail.txt", chunks)
            .await
            .unwrap_err();

        assert!(matches!(err, ApiError::InternalFailure()));
    }
}
