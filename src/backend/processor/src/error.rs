use minidump_processor::ProcessError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JobError {
    #[error("internal failure")]
    InternalFailure(),

    #[error("general failure: `{0}'")]
    Failure(String),

    #[error("storage error: `{0}'")]
    StorageError(#[from] object_store::Error),

    #[error("failed to process minidump: `{0}'")]
    MinidumpError(#[from] minidump::Error),

    #[error("failed to process minidump: `{0}'")]
    MinidumpProcessError(#[from] ProcessError),

    #[error("Regex error: {0}")]
    Regex(Box<fancy_regex::Error>),

    #[error("apalis error: `{0}'")]
    ApalisError(String),
}

impl From<fancy_regex::Error> for JobError {
    fn from(error: fancy_regex::Error) -> Self {
        JobError::Regex(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_job_error_variants() {
        assert_eq!(JobError::InternalFailure().to_string(), "internal failure");
        assert_eq!(JobError::Failure("bad".to_string()).to_string(), "general failure: `bad'");
        assert_eq!(JobError::ApalisError("queue".to_string()).to_string(), "apalis error: `queue'");
    }

    #[test]
    fn converts_regex_errors() {
        let err: JobError = fancy_regex::Regex::new("(").unwrap_err().into();
        assert!(matches!(err, JobError::Regex(_)));
        assert!(err.to_string().contains("Regex error"));
    }
}
