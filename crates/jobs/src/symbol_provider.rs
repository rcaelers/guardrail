use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use breakpad_symbols::lookup;
use minidump::Module;
use minidump_unwind::{
    FileError, FileKind, LocateSymbolsResult, SymbolError, SymbolFile, SymbolSupplier,
};
use object_store::ObjectStore;
use tracing::{info, trace};

pub fn s3_symbol_supplier(storage: Arc<dyn ObjectStore>) -> impl SymbolSupplier {
    S3SymbolSupplier::new(storage)
}

pub struct S3SymbolSupplier {
    pub storage: Arc<dyn ObjectStore>,
}

impl S3SymbolSupplier {
    pub fn new(storage: Arc<dyn ObjectStore>) -> S3SymbolSupplier {
        S3SymbolSupplier { storage }
    }
}

#[async_trait]
impl SymbolSupplier for S3SymbolSupplier {
    async fn locate_symbols(
        &self,
        module: &(dyn Module + Sync),
    ) -> Result<LocateSymbolsResult, SymbolError> {
        info!(
            "locate_symbols {} {} {} {} {} {} {}",
            module.base_address(),
            module.size(),
            module.code_file(),
            module.code_identifier().unwrap_or_default(),
            module.debug_file().unwrap_or_default(),
            module.debug_identifier().unwrap_or_default(),
            module.version().unwrap_or_default()
        );

        let file_path = self
            .locate_file(module, FileKind::BreakpadSym)
            .await
            .map_err(|_| SymbolError::NotFound)?;

        trace!("S3SymbolSupplier found file {:?}", file_path);
        // let symbols = SymbolFile::from_file(&file_path).map_err(|e| {
        //     trace!("S3SymbolSupplier failed: {}", e);
        //     e
        // })?;
        // trace!("S3SymbolSupplier parsed file!");
        // Ok(LocateSymbolsResult {
        //     symbols,
        //     extra_debug_info: None,
        // })
        Err(SymbolError::NotFound)
    }

    async fn locate_file(
        &self,
        module: &(dyn Module + Sync),
        file_kind: FileKind,
    ) -> Result<PathBuf, FileError> {
        trace!("SimpleSymbolSupplier search");
        if let Some(lookup) = lookup(module, file_kind) {
            trace!("SimpleSymbolSupplier found lookup {:?}", lookup);
            // let test_path = path.join(lookup.cache_rel.clone());
            // if fs::metadata(&test_path).ok().is_some_and(|m| m.is_file()) {
            //     trace!("SimpleSymbolSupplier found file {}", test_path.display());
            //     return Ok(test_path);
            // }
        }
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
    use testware::{
        create_settings, create_test_product_with_details, create_test_version, setup::TestSetup,
    };

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_s3_symbol_supplier(pool: PgPool) {
        TestSetup::init();
        //let settings = create_settings();
        //let repo = Repo::new(pool.clone());
        let store = Arc::new(object_store::memory::InMemory::new());
        //let worker = Arc::new(TestMinidumpProcessor::new());

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

        // MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb

        let module_id = "EE9E2672A6863B084C4C44205044422E1".to_string();
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

        let provider = Symbolizer::new(s3_symbol_supplier(store));

        let state = minidump_processor::process_minidump_with_options(&dump, &provider, options)
            .await
            .expect("Failed to process minidump");

        // let mut json_output = Vec::new();
        // state.print_json(&mut json_output, false).map_err(|e| {
        //     error!("Failed to print minidump json: {:?}", e);
        //     JobError::Failure("failed to print minidump json".to_string())
        // })?;
        // let json: Value = serde_json::from_slice(&json_output).map_err(|e| {
        //     error!("Failed to parse minidump json: {:?}", e);
        //     JobError::Failure("failed to parse minidump json".to_string())
        // })?;
    }
}
