use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtilsError {
    #[error("general failure")]
    Failure,

    #[error("io-error: `{0}`")]
    IOError(#[from] std::io::Error),
}
