use serde::{Deserialize, Serialize};

/// Job queued by API when a minidump is uploaded.
/// Consumed by the processor.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinidumpJob {
    pub crash: serde_json::Value,
}

/// Job queued by API when symbols are uploaded.
/// Consumed by the processor for validation.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SymbolJob {
    pub symbol_info: serde_json::Value,
}

/// Job queued by the processor after processing a minidump.
/// Consumed by the curator to import into the database.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImportCrashJob {
    pub crash_id: String,
}

/// Job queued by the processor after validating symbols.
/// Consumed by the curator to import metadata into the database.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImportSymbolJob {
    pub symbol_upload_id: String,
}

pub mod queue {
    pub const MINIDUMP_JOBS: &str = "guardrail::MinidumpJobs";
    pub const SYMBOL_JOBS: &str = "guardrail::SymbolJobs";
    pub const IMPORT_CRASH_JOBS: &str = "guardrail::ImportCrashJobs";
    pub const IMPORT_SYMBOL_JOBS: &str = "guardrail::ImportSymbolJobs";
}
