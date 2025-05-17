use async_trait::async_trait;
use futures::stream::BoxStream;
use mockall::mock;
use mockall::predicate::*;
use object_store::{
    GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, ObjectStore as OSObjectStore,
    PutMultipartOpts, PutOptions, PutPayload, PutResult, Result, path::Path,
};
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

mock! {
    pub ObjectStore {}

    #[async_trait]
    impl OSObjectStore for ObjectStore {
        async fn put(&self, location: &Path, payload: PutPayload) -> Result<PutResult>;
        async fn put_opts(&self, location: &Path,  payload: PutPayload, opts: PutOptions) -> Result<PutResult>;
        async fn put_multipart_opts(&self, location: &Path, opts: PutMultipartOpts) -> Result<Box<dyn MultipartUpload>>;
        async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult>;
        async fn delete(&self, location: &Path) -> Result<()>;
        fn list<'a>(&'a self, prefix: Option<&'a Path>) -> BoxStream<'static, Result<ObjectMeta>>;
        async fn list_with_delimiter<'a, 'b>(&'a self, prefix: Option<&'b Path>) -> Result<ListResult>;
        async fn copy(&self, from: &Path, to: &Path) -> Result<()>;
        async fn copy_if_not_exists(&self, from: &Path, to: &Path) -> Result<()>;
    }
}

impl std::fmt::Debug for MockObjectStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "MockObjectStore")
    }
}

impl Display for MockObjectStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "MockObjectStore")
    }
}

#[derive(Clone, Debug)]
pub struct MockObjectStoreWrapper {
    inner: Arc<MockObjectStore>,
}

impl Display for MockObjectStoreWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "MockObjectStoreWrapper")
    }
}

impl MockObjectStoreWrapper {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MockObjectStore::new()),
        }
    }

    pub fn mock(&self) -> Arc<MockObjectStore> {
        self.inner.clone()
    }
}

impl Default for MockObjectStoreWrapper {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OSObjectStore for MockObjectStoreWrapper {
    async fn put(&self, location: &Path, payload: PutPayload) -> Result<PutResult> {
        self.inner.put(location, payload).await
    }

    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> Result<PutResult> {
        self.inner.put_opts(location, payload, opts).await
    }

    async fn put_multipart(&self, location: &Path) -> Result<Box<dyn MultipartUpload>> {
        self.inner.put_multipart(location).await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOpts,
    ) -> Result<Box<dyn MultipartUpload>> {
        self.inner.put_multipart_opts(location, opts).await
    }

    async fn get(&self, location: &Path) -> Result<GetResult> {
        self.inner.get(location).await
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        self.inner.get_opts(location, options).await
    }

    async fn head(&self, location: &Path) -> Result<ObjectMeta> {
        self.inner.head(location).await
    }

    async fn delete(&self, location: &Path) -> Result<()> {
        self.inner.delete(location).await
    }

    fn list<'a>(&'a self, prefix: Option<&'a Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        self.inner.list(prefix)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        self.inner.list_with_delimiter(prefix).await
    }

    async fn copy(&self, from: &Path, to: &Path) -> Result<()> {
        self.inner.copy(from, to).await
    }

    async fn copy_if_not_exists(&self, from: &Path, to: &Path) -> Result<()> {
        self.inner.copy_if_not_exists(from, to).await
    }
}
