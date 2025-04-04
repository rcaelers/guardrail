use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{debug, error};

#[derive(Debug, Default)]
pub struct FileCleanupTracker {
    files_to_cleanup: HashSet<PathBuf>,
}

impl FileCleanupTracker {
    pub fn new() -> Self {
        Self {
            files_to_cleanup: HashSet::new(),
        }
    }

    pub fn track_file(&mut self, path: PathBuf) {
        self.files_to_cleanup.insert(path);
    }

    pub async fn cleanup_all(&self) {
        for file_path in &self.files_to_cleanup {
            if let Err(e) = tokio::fs::remove_file(file_path).await {
                error!("Failed to cleanup file during rollback: {:?} - {:?}", file_path, e);
            } else {
                debug!("Successfully cleaned up file during rollback: {:?}", file_path);
            }
        }
    }
}
