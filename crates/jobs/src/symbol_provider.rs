use async_trait::async_trait;
use bytes::Bytes;
use data::symbols::Symbols;
use minidump::Module;
use minidump_unwind::{
    FileError, FileKind, LocateSymbolsResult, SymbolError, SymbolFile, SymbolSupplier,
};
use object_store::{ObjectStore, path::Path};
use repos::{Repo, symbols::SymbolsRepo};
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info};

pub fn s3_symbol_supplier(storage: Arc<dyn ObjectStore>, repo: Repo) -> impl SymbolSupplier {
    S3SymbolSupplier::new(storage, repo)
}

pub struct S3SymbolSupplier {
    pub storage: Arc<dyn ObjectStore>,
    pub repo: Repo,
}

impl S3SymbolSupplier {
    pub fn new(storage: Arc<dyn ObjectStore>, repo: Repo) -> S3SymbolSupplier {
        S3SymbolSupplier { storage, repo }
    }

    async fn get_symbols_by_module_and_build_id(
        &self,
        module_id: &str,
        build_id: &str,
    ) -> Result<Symbols, SymbolError> {
        let mut conn = self.repo.acquire_admin().await.map_err(|err| {
            error!("Failed to acquire connection: {err}");
            SymbolError::NotFound
        })?;

        let symbol = SymbolsRepo::get_by_module_and_build_id(
            &mut *conn,
            build_id,
            module_id,
        )
        .await
        .map_err(|err| {
            error!(
                "Failed to retrieve symbols for build_id: {build_id}, module_id: {module_id}: {err}"
            );
            SymbolError::NotFound
        })?
        .ok_or_else(|| {
            error!(
                "Failed to retrieve symbols for build_id: {build_id}, module_id: {module_id}"
            );
            SymbolError::NotFound
        })?;
        Ok(symbol)
    }

    async fn get_symbols_object(&self, path: &str) -> Result<Bytes, SymbolError> {
        let object = self.storage.get(&Path::from(path)).await.map_err(|err| {
            error!("Failed to get symbols object: {err}");
            SymbolError::NotFound
        })?;
        info!("Got symbols object: {:?}", object);
        let data = object.bytes().await.map_err(|err| {
            error!("Failed to read symbols object: {err}");
            SymbolError::NotFound
        })?;
        Ok(data)
    }

    async fn parse_symbols(&self, data: &[u8]) -> Result<SymbolFile, SymbolError> {
        SymbolFile::from_bytes(data).map_err(|e| {
            error!("Failed to parse symbols: {}", e);
            SymbolError::NotFound
        })
    }
}

fn convert(s: &str) -> &str {
    s
}

#[async_trait]
impl SymbolSupplier for S3SymbolSupplier {
    async fn locate_symbols(
        &self,
        module: &(dyn Module + Sync),
    ) -> Result<LocateSymbolsResult, SymbolError> {
        let build_id = module.debug_identifier().ok_or(SymbolError::NotFound)?;
        let build_id = build_id.breakpad().to_string();
        let module_id = module.debug_file().ok_or(SymbolError::NotFound)?;
        let module_id = std::path::Path::new(convert(module_id.as_ref()))
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(SymbolError::NotFound)?
            .to_string();

        info!("Searching symbols for module_id: {}, build_id: {}", module_id, build_id);

        let symbols = self
            .get_symbols_by_module_and_build_id(&module_id, &build_id)
            .await?;
        let data = self.get_symbols_object(&symbols.file_location).await?;
        let symbols = self.parse_symbols(&data).await?;

        info!("S3SymbolSupplier parsed file!");
        Ok(LocateSymbolsResult {
            symbols,
            extra_debug_info: None,
        })
    }

    async fn locate_file(
        &self,
        module: &(dyn Module + Sync),
        file_kind: FileKind,
    ) -> Result<PathBuf, FileError> {
        info!(
            "S3SymbolSupplier locate_file {:?} {}",
            file_kind,
            module.debug_file().unwrap_or_default()
        );
        Err(FileError::NotFound)
    }
}

#[cfg(test)]
mod test {
    use data::symbols::NewSymbols;
    use minidump::Minidump;
    use minidump_processor::ProcessorOptions;
    use minidump_unwind::Symbolizer;
    use object_store::{PutPayload, path::Path};
    use sqlx::PgPool;
    use std::sync::Arc;

    use super::*;
    use repos::{Repo, symbols::SymbolsRepo};
    use testware::{create_test_product_with_details, create_test_version, setup::TestSetup};

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_s3_symbol_supplier(pool: PgPool) {
        TestSetup::init();
        let repo = Repo::new(pool.clone());
        let store = Arc::new(object_store::memory::InMemory::new());

        let product =
            create_test_product_with_details(&pool, "TestProduct", "Test product description")
                .await;
        let version =
            create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

        let workspace_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().expect("Failed to get current directory"))
            .ancestors()
            .nth(2)
            .expect("Failed to find workspace root")
            .to_path_buf();

        let path = workspace_dir.join("dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp");
        info!("minidump path: {:?}", path);
        let dump = Minidump::read_path(path).unwrap();

        let module_id = "crash.pdb".to_string();
        let build_id = "EE9E2672A6863B084C4C44205044422E1".to_string();
        let symbols_path = format!("symbols/{}-{}", module_id, build_id);
        let data = NewSymbols {
            build_id,
            module_id,
            file_location: symbols_path.clone(),
            product_id: product.id,
            version_id: version.id,
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
        };

        let path = workspace_dir.join("dev/crash.sym");
        let payload = tokio::fs::read(path)
            .await
            .map(PutPayload::from)
            .expect("Failed to read symbol file");
        store
            .put(&Path::from(symbols_path), payload)
            .await
            .expect("Failed to put symbols");

        let _symbol_id = SymbolsRepo::create(&pool, data)
            .await
            .expect("Failed to create symbol");

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let provider = Symbolizer::new(s3_symbol_supplier(store, repo));
        let state = minidump_processor::process_minidump_with_options(&dump, &provider, options)
            .await
            .expect("Failed to process minidump");

        let mut json_output = Vec::new();
        state
            .print_json(&mut json_output, false)
            .expect("Failed to print json");
        let json_str = String::from_utf8_lossy(&json_output);
        let json: serde_json::Value =
            serde_json::from_str(&json_str).expect("Failed to parse json");
        info!(
            "json_output pretty: {}",
            serde_json::to_string_pretty(&json).expect("Failed to format json")
        );

        assert!(json["crashing_thread"].is_object());
        assert!(json["crashing_thread"]["frames"].is_array());
        assert!(json["crashing_thread"]["frames"][0]["missing_symbols"].is_boolean());
        assert!(
            !json["crashing_thread"]["frames"][0]["missing_symbols"]
                .as_bool()
                .unwrap()
        );
        assert_eq!(
            json["crashing_thread"]["frames"][0]["module"]
                .as_str()
                .unwrap(),
            "crash.exe"
        );
        assert_eq!(
            json["crashing_thread"]["frames"][0]["function"]
                .as_str()
                .unwrap(),
            "crash2()"
        );
        assert_eq!(
            json["crashing_thread"]["frames"][4]["function"]
                .as_str()
                .unwrap(),
            "main(int, char**)"
        );
    }
}
