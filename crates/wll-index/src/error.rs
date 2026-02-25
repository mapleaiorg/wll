//! Error types for the index crate.

use wll_types::ObjectId;

/// Errors that can occur during index operations.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// The specified path was not found in the index.
    #[error("path not found in index: {0}")]
    PathNotFound(String),

    /// The specified path is already staged.
    #[error("path already staged: {0}")]
    AlreadyStaged(String),

    /// An object referenced by the index was not found in the store.
    #[error("object not found in store: {0:?}")]
    ObjectNotFound(ObjectId),

    /// The entry has a conflict that must be resolved first.
    #[error("unresolved conflict at path: {0}")]
    UnresolvedConflict(String),

    /// Store operation failed.
    #[error("store error: {0}")]
    Store(#[from] wll_store::StoreError),

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// An invalid path was provided.
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

/// Convenience alias for index results.
pub type IndexResult<T> = Result<T, IndexError>;
