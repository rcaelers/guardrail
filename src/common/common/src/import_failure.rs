use serde::{Deserialize, Serialize};

pub const ACTIVE_IMPORT_RETRY_SECONDS: i64 = 60;
pub const STALLED_IMPORT_AGE_SECONDS: i64 = 300;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    Crash,
    Symbol,
}

impl ImportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::Symbol => "symbol",
        }
    }

    pub fn source_path(self, id: &str) -> String {
        match self {
            Self::Crash => format!("processed-crashes/{id}.json"),
            Self::Symbol => format!("processed-symbols/{id}.json"),
        }
    }

    pub fn failure_path(self, id: &str) -> String {
        format!("import-failures/{}/{id}.json", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportFailureStatus {
    Failed,
    Retrying,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub id: String,
    pub kind: ImportKind,
    pub product_id: String,
    pub subject: String,
    pub error: String,
    pub attempts: u32,
    pub first_failed_at: String,
    pub last_failed_at: String,
    pub status: ImportFailureStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_paths_are_stable_and_separate() {
        assert_eq!(ImportKind::Crash.source_path("abc"), "processed-crashes/abc.json");
        assert_eq!(ImportKind::Symbol.source_path("abc"), "processed-symbols/abc.json");
        assert_eq!(ImportKind::Crash.failure_path("abc"), "import-failures/crash/abc.json");
        assert_eq!(ImportKind::Symbol.failure_path("abc"), "import-failures/symbol/abc.json");
    }
}
