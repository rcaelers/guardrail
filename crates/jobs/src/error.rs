use minidump_processor::ProcessError;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum JobError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure: `{0}'")]
    Failure(String),

    #[error("database error: `{0}'")]
    RepoError(#[from] repos::error::RepoError),

    #[error("storage error: `{0}'")]
    StorageError(#[from] object_store::Error),

    #[error("failed to process minidump: `{0}'")]
    MinidumpError(#[from] minidump::Error),

    #[error("failed to process minidump: `{0}'")]
    MinidumpProcessError(#[from] ProcessError),

    #[error("Regex error: {0}")]
    Regex(Box<fancy_regex::Error>),
}

impl From<fancy_regex::Error> for JobError {
    fn from(error: fancy_regex::Error) -> Self {
        JobError::Regex(Box::new(error))
    }
}
