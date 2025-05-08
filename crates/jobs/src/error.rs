use minidump_processor::ProcessError;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum JobError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure")]
    Failure(String),

    #[error("database error: `{0}`")]
    RepoError(#[from] repos::error::RepoError),

    #[error("failed to process minidump: `{0}`")]
    MinidumpError(#[from] minidump::Error),

    #[error("failed to process minidump: `{0}`")]
    MinidumpProcessError(#[from] ProcessError),
}
