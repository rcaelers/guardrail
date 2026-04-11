use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("database failure: {0}")]
    DatabaseError(String),

    #[error("database invalid column: {0}")]
    InvalidColumn(String),

    #[error("not found")]
    NotFound(),

    #[error("database uniqueness violation")]
    UniqueViolation(String, String),

    #[error("database foreign key violation")]
    ForeignKeyViolation(String, String),

    #[error("database integrity check")]
    CheckViolation(String, String),

    #[error("database transaction error")]
    TransactionError(),

    #[error("database error")]
    Other(),
}

pub fn handle_surreal_error(err: surrealdb::Error) -> RepoError {
    error!("SurrealDB error: {}", err);
    let msg = err.to_string();

    if msg.contains("already exists") || msg.contains("unique") {
        return RepoError::UniqueViolation(String::new(), msg);
    }

    RepoError::DatabaseError(msg)
}
