use axum::BoxError;
use axum::body::Bytes;
use axum::extract::multipart::Field;
use futures::{Stream, StreamExt, TryStreamExt};
use object_store::{ObjectStore, path::Path};
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::io::AsyncWriteExt;
use tokio::io::{self};
use tokio_util::io::StreamReader;
use tracing::error;

use crate::error::ApiError;
use data::api_token::ApiToken;
use data::product::Product;
use repos::product::ProductRepo;

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

pub async fn get_product_by_name(
    db: &Surreal<Any>,
    product_name: &str,
) -> Result<Product, ApiError> {
    ProductRepo::get_by_name(db, product_name)
        .await
        .map_err(|err| {
            error!("Failed to get product by name {}: {}", product_name, err);
            ApiError::Failure(format!("failed to get product by name {product_name}"))
        })?
        .ok_or_else(|| ApiError::ProductNotFound(product_name.to_string()))
}

pub async fn get_product_by_id(db: &Surreal<Any>, product_id: &str) -> Result<Product, ApiError> {
    ProductRepo::get_by_id(db, product_id)
        .await
        .map_err(|err| {
            error!("Failed to get product by id {}: {}", product_id, err);
            ApiError::Failure(format!("failed to get product by id {product_id}"))
        })?
        .ok_or_else(|| ApiError::ProductNotFound(product_id.to_string()))
}

pub async fn get_product_by_ingestion_token(
    db: &Surreal<Any>,
    token: &str,
) -> Result<Option<Product>, ApiError> {
    ProductRepo::get_by_ingestion_token(db, token)
        .await
        .map_err(|err| {
            error!("Failed to look up product by ingestion token: {}", err);
            ApiError::InternalFailure()
        })
}

pub fn validate_api_token_for_product(
    api_token: &ApiToken,
    product: &Product,
    product_name: &str,
) -> Result<(), ApiError> {
    if let Some(token_product_id) = api_token.product_id.as_deref()
        && token_product_id != product.id
    {
        error!(
            "API token not authorized for product {}, token is for product_id: {} not {}",
            product_name, token_product_id, product.id
        );
        return Err(ApiError::ProductAccessDenied(product_name.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::{FromRequest, Multipart};
    use axum::http::{Request, header::CONTENT_TYPE};
    use bytes::Bytes;
    use chrono::Utc;
    use futures::{TryStreamExt, stream};
    use object_store::memory::InMemory;
    use uuid::Uuid;

    fn token(product_id: Option<&str>) -> ApiToken {
        ApiToken {
            id: "api_tokens:test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            description: "test".to_string(),
            token_id: Uuid::new_v4(),
            token_hash: "hash".to_string(),
            product_id: product_id.map(str::to_string),
            user_id: None,
            entitlements: vec!["symbol-upload".to_string()],
            last_used_at: None,
            expires_at: None,
            is_active: true,
        }
    }

    fn product(id: &str) -> Product {
        Product {
            id: id.to_string(),
            name: "TestProduct".to_string(),
            slug: "test-product".to_string(),
            description: String::new(),
            public: false,
            accepting_crashes: true,
            ingestion_token: String::new(),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

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
        let chunks = stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"partial")),
            Err(std::io::Error::other("stream failed")),
        ]);

        let err = stream_to_s3(Arc::new(InMemory::new()), "objects/test.txt", chunks)
            .await
            .unwrap_err();

        assert!(matches!(err, ApiError::InternalFailure()));
    }

    #[tokio::test]
    async fn peek_line_returns_first_line_and_replays_stream() {
        let boundary = "----guardrail-api-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"app.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE Linux x86 AABBCC app.pdb\nPUBLIC 1 0 main\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let mut multipart = Multipart::from_request(request, &()).await.unwrap();
        let field = multipart.next_field().await.unwrap().unwrap();

        let (line, stream) = peek_line(field).await.unwrap();
        let bytes = stream
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await
            .unwrap();

        assert_eq!(line, "MODULE Linux x86 AABBCC app.pdb\n");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "MODULE Linux x86 AABBCC app.pdb\nPUBLIC 1 0 main"
        );
    }

    #[tokio::test]
    async fn peek_line_reports_missing_newline() {
        let boundary = "----guardrail-api-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"app.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE without newline\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let mut multipart = Multipart::from_request(request, &()).await.unwrap();
        let field = multipart.next_field().await.unwrap().unwrap();

        let error = match peek_line(field).await {
            Ok(_) => panic!("expected missing newline error"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(error.to_string(), "No complete line found");
    }

    #[test]
    fn validate_api_token_allows_global_or_matching_product_tokens() {
        let product = product("products:one");

        assert!(validate_api_token_for_product(&token(None), &product, "TestProduct").is_ok());
        assert!(
            validate_api_token_for_product(&token(Some("products:one")), &product, "TestProduct")
                .is_ok()
        );
    }

    #[test]
    fn validate_api_token_rejects_other_products() {
        let product = product("products:one");

        assert!(matches!(
            validate_api_token_for_product(
                &token(Some("products:two")),
                &product,
                "TestProduct"
            ),
            Err(ApiError::ProductAccessDenied(product)) if product == "TestProduct"
        ));
    }
}
