use thiserror::Error;

/// Errors produced by type operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypeError {
    #[error("invalid hex string: {0}")]
    InvalidHex(String),

    #[error("invalid byte length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },

    #[error("serialization error: {0}")]
    Serialization(String),
}
