use async_trait::async_trait;
use bytes::Bytes;
use minidump::Module;
use minidump_unwind::{
    FileError, FileKind, LocateSymbolsResult, SymbolError, SymbolFile, SymbolSupplier,
};
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::{path::PathBuf, sync::Arc};
use tracing::{debug, error, info};

pub struct S3SymbolSupplier {
    pub storage: Arc<dyn ObjectStore>,
}

impl S3SymbolSupplier {
    pub fn new(storage: Arc<dyn ObjectStore>) -> S3SymbolSupplier {
        S3SymbolSupplier { storage }
    }

    async fn get_symbols_object(&self, path: &str) -> Result<Bytes, SymbolError> {
        let object = self.storage.get(&Path::from(path)).await.map_err(|err| {
            debug!("Symbols object not found at {}: {err}", path);
            SymbolError::NotFound
        })?;
        info!("Got symbols object: {:?}", object);
        let data = object.bytes().await.map_err(|err| {
            debug!("Failed to read symbols object at {}: {err}", path);
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

/// `module_id` and `build_id` come from the attacker-controlled minidump and are
/// interpolated into object-store keys. Reject anything that is not a safe single
/// path segment so a crafted module can't address objects outside the `symbols/`
/// prefix — notably on filesystem-backed stores, where a `..` segment traverses.
///
/// This rejects rather than rewrites: rewriting on the read path (as the upload
/// side does) would mismatch symbols legitimately stored under names containing
/// characters like `+` (e.g. `libstdc++`).
fn is_safe_path_segment(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains('/') && !s.contains('\\')
}

/// A PE with no CodeView record reports a nil debug id, which every build of
/// that DLL shares. The MSYS2/MinGW GTK libraries are all like this, so the
/// code id -- the PE timestamp and image size -- is the only thing that tells
/// their builds apart. Prefer it, and keep the nil id as a fallback so symbols
/// uploaded under it are still found.
fn code_id_of(module: &(dyn Module + Sync)) -> Option<String> {
    let code_id = module.code_identifier()?;
    let code_id = code_id.to_string().to_uppercase();
    (!code_id.is_empty()).then_some(code_id)
}

impl S3SymbolSupplier {
    fn lookup_ids(module: &(dyn Module + Sync)) -> Vec<String> {
        match module.debug_identifier() {
            Some(debug_id) if !debug_id.is_nil() => vec![debug_id.breakpad().to_string()],
            Some(debug_id) => code_id_of(module)
                .into_iter()
                .chain(std::iter::once(debug_id.breakpad().to_string()))
                .collect(),
            None => code_id_of(module).into_iter().collect(),
        }
    }
}

#[async_trait]
impl SymbolSupplier for S3SymbolSupplier {
    async fn locate_symbols(
        &self,
        module: &(dyn Module + Sync),
    ) -> Result<LocateSymbolsResult, SymbolError> {
        let build_ids = Self::lookup_ids(module);
        if build_ids.is_empty() {
            return Err(SymbolError::NotFound);
        }
        let module_id = module.debug_file().ok_or(SymbolError::NotFound)?;
        let module_id = std::path::Path::new(convert(module_id.as_ref()))
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(SymbolError::NotFound)?
            .to_string();

        if !is_safe_path_segment(&module_id) {
            error!(
                module_id = %module_id,
                "Rejecting unsafe symbol path segment from minidump"
            );
            return Err(SymbolError::NotFound);
        }

        for build_id in &build_ids {
            if !is_safe_path_segment(build_id) {
                error!(
                    module_id = %module_id,
                    build_id = %build_id,
                    "Rejecting unsafe symbol path segment from minidump"
                );
                continue;
            }

            info!("Searching symbols for module_id: {}, build_id: {}", module_id, build_id);

            // Standard Breakpad layout first, then the flat one guardrail also writes.
            let paths = [
                format!("symbols/{}/{}/{}.sym", module_id, build_id, module_id),
                format!("symbols/{}-{}", module_id, build_id),
            ];
            for path in &paths {
                match self.get_symbols_object(path).await {
                    Ok(data) => {
                        let symbols = self.parse_symbols(&data).await?;
                        info!("S3SymbolSupplier parsed file from: {}", path);
                        return Ok(LocateSymbolsResult {
                            symbols,
                            extra_debug_info: None,
                        });
                    }
                    Err(_) => debug!("No symbols at {}", path),
                }
            }
        }

        Err(SymbolError::NotFound)
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
    use minidump::Minidump;
    use minidump_processor::ProcessorOptions;
    use minidump_unwind::Symbolizer;
    use object_store::{PutPayload, path::Path};
    use std::{path::PathBuf, sync::Arc};
    use tracing::info;

    use super::*;

    #[test]
    fn is_safe_path_segment_rejects_traversal_and_separators() {
        assert!(super::is_safe_path_segment("crash.pdb"));
        assert!(super::is_safe_path_segment("libstdc++.so.6"));
        assert!(super::is_safe_path_segment("EE9E2672A6863B084C4C44205044422E1"));
        assert!(!super::is_safe_path_segment(""));
        assert!(!super::is_safe_path_segment("."));
        assert!(!super::is_safe_path_segment(".."));
        assert!(!super::is_safe_path_segment("a/b"));
        assert!(!super::is_safe_path_segment("a\\b"));
    }

    #[tokio::test]
    async fn get_symbols_object_and_parse_symbols_report_failures() {
        let supplier = S3SymbolSupplier::new(Arc::new(object_store::memory::InMemory::new()));

        assert!(matches!(
            supplier.get_symbols_object("symbols/missing.sym").await,
            Err(SymbolError::NotFound)
        ));
        assert!(matches!(
            supplier.parse_symbols(b"not a breakpad symbol file").await,
            Err(SymbolError::NotFound)
        ));
    }

    #[tokio::test]
    async fn parse_symbols_accepts_minimal_breakpad_module() {
        let supplier = S3SymbolSupplier::new(Arc::new(object_store::memory::InMemory::new()));
        supplier
            .parse_symbols(b"MODULE Linux x86 ABCDEF test\n")
            .await
            .expect("minimal symbol file should parse");
    }

    /// A stand-in for a MinGW DLL: no CodeView record, so a nil debug id.
    struct FakeModule {
        debug_id: Option<debugid::DebugId>,
        code_id: Option<debugid::CodeId>,
    }

    impl minidump::Module for FakeModule {
        fn base_address(&self) -> u64 { 0 }
        fn size(&self) -> u64 { 0x1000 }
        fn code_file(&self) -> std::borrow::Cow<'_, str> { "libglib-2.0-0.dll".into() }
        fn code_identifier(&self) -> Option<debugid::CodeId> { self.code_id.clone() }
        fn debug_file(&self) -> Option<std::borrow::Cow<'_, str>> { Some("libglib-2.0-0.dll".into()) }
        fn debug_identifier(&self) -> Option<debugid::DebugId> { self.debug_id }
        fn version(&self) -> Option<std::borrow::Cow<'_, str>> { None }
    }

    #[test]
    fn a_nil_debug_id_falls_back_to_the_code_id() {
        let nil = debugid::DebugId::nil();
        let code = debugid::CodeId::new("6a3f7c5516c000".to_string());

        // A real debug id is used on its own; the code id must not shadow it.
        let real = debugid::DebugId::from_breakpad("94A7D9F01A528C944C4C44205044422E1").unwrap();
        assert_eq!(
            S3SymbolSupplier::lookup_ids(&FakeModule { debug_id: Some(real), code_id: Some(code.clone()) }),
            vec!["94A7D9F01A528C944C4C44205044422E1".to_string()]
        );

        // A nil debug id: try the code id first, then the nil id so anything
        // already uploaded under it is still found.
        assert_eq!(
            S3SymbolSupplier::lookup_ids(&FakeModule { debug_id: Some(nil), code_id: Some(code.clone()) }),
            vec![
                "6A3F7C5516C000".to_string(),
                "000000000000000000000000000000000".to_string()
            ],
            "the code id is uppercased to match how build ids are stored"
        );

        // Nil debug id and no code id at all: only the nil id is left.
        assert_eq!(
            S3SymbolSupplier::lookup_ids(&FakeModule { debug_id: Some(nil), code_id: None }),
            vec!["000000000000000000000000000000000".to_string()]
        );

        // No debug id recorded at all.
        assert_eq!(
            S3SymbolSupplier::lookup_ids(&FakeModule { debug_id: None, code_id: Some(code) }),
            vec!["6A3F7C5516C000".to_string()]
        );
        assert!(S3SymbolSupplier::lookup_ids(&FakeModule { debug_id: None, code_id: None }).is_empty());
    }

    #[tokio::test]
    async fn symbols_are_found_under_the_code_id() {
        let store = Arc::new(object_store::memory::InMemory::new());
        let sym = b"MODULE windows x86_64 6A3F7C5516C000 libglib-2.0-0.dll\nPUBLIC 1330 0 g_mem_chunk_new\n";
        store
            .put(
                &object_store::path::Path::from(
                    "symbols/libglib-2.0-0.dll/6A3F7C5516C000/libglib-2.0-0.dll.sym",
                ),
                sym.to_vec().into(),
            )
            .await
            .unwrap();

        let supplier = S3SymbolSupplier::new(store);
        let module = FakeModule {
            debug_id: Some(debugid::DebugId::nil()),
            code_id: Some(debugid::CodeId::new("6a3f7c5516c000".to_string())),
        };
        let found = supplier.locate_symbols(&module).await.expect("code id lookup must resolve");
        assert_eq!(found.symbols.publics.len(), 1);
    }

    #[tokio::test]
    async fn test_s3_symbol_supplier_standard_path() {
        let store = Arc::new(object_store::memory::InMemory::new());

        let workspace_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().expect("Failed to get current directory"))
            .ancestors()
            .nth(3)
            .expect("Failed to find workspace root")
            .to_path_buf();

        let minidump_path = workspace_dir.join("dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp");
        info!("minidump path: {:?}", minidump_path);
        let dump = Minidump::read_path(minidump_path).unwrap();

        let module_id = "crash.pdb";
        let build_id = "EE9E2672A6863B084C4C44205044422E1";

        // Use standard Breakpad path structure: symbols/{module_id}/{build_id}/{module_id}.sym
        let symbols_path = format!("symbols/{}/{}/{}.sym", module_id, build_id, module_id);

        let symbol_file_path = workspace_dir.join("dev/crash.sym");
        let payload = tokio::fs::read(symbol_file_path)
            .await
            .map(PutPayload::from)
            .expect("Failed to read symbol file");
        store
            .put(&Path::from(symbols_path), payload)
            .await
            .expect("Failed to put symbols");

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let provider = Symbolizer::new(S3SymbolSupplier::new(store));
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

    #[tokio::test]
    async fn test_s3_symbol_supplier_alternate_path() {
        let store = Arc::new(object_store::memory::InMemory::new());

        let workspace_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().expect("Failed to get current directory"))
            .ancestors()
            .nth(3)
            .expect("Failed to find workspace root")
            .to_path_buf();

        let minidump_path = workspace_dir.join("dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp");
        info!("minidump path: {:?}", minidump_path);
        let dump = Minidump::read_path(minidump_path).unwrap();

        let module_id = "crash.pdb";
        let build_id = "EE9E2672A6863B084C4C44205044422E1";

        // Use alternate path format: symbols/{module_id}-{build_id}
        let symbols_path = format!("symbols/{}-{}", module_id, build_id);

        let symbol_file_path = workspace_dir.join("dev/crash.sym");
        let payload = tokio::fs::read(symbol_file_path)
            .await
            .map(PutPayload::from)
            .expect("Failed to read symbol file");
        store
            .put(&Path::from(symbols_path), payload)
            .await
            .expect("Failed to put symbols");

        let mut options = ProcessorOptions::default();
        options.recover_function_args = true;

        let provider = Symbolizer::new(S3SymbolSupplier::new(store));
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
