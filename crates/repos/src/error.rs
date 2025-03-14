use thiserror::Error;

#[derive(Error, Clone, Debug)]
pub enum RepoError {
    #[error("Database failure: {0}")]
    DatabaseError(String),

    #[error("Invalid column: {0}")]
    InvalidColumn(String),
}
