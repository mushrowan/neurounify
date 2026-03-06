use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("invalid header: {0}")]
    InvalidHeader(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(PathBuf),

    #[error("encoding error: {0}")]
    Encoding(String),
}

pub type Result<T> = std::result::Result<T, Error>;
