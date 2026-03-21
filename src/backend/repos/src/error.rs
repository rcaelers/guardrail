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

pub fn handle_sql_error(err: sqlx::Error) -> RepoError {
    use sqlx::Error as E;

    error!("SQL error: {}", err);
    match err {
        E::RowNotFound => RepoError::NotFound(),
        E::Database(ref e) => {
            let constraint = e.constraint().unwrap_or_default().to_string();
            let table = e.table().unwrap_or_default().to_string();

            if e.is_unique_violation() {
                return RepoError::UniqueViolation(table, constraint);
            }

            if e.is_foreign_key_violation() {
                return RepoError::ForeignKeyViolation(table, constraint);
            }

            if e.is_check_violation() {
                return RepoError::CheckViolation(table, constraint);
            }

            RepoError::Other()
        }
        _ => RepoError::Other(),
    }
}
