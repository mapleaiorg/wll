//! Error types for the diff crate.

use wll_types::ObjectId;

/// Errors that can occur during diff operations.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    /// An object referenced during diff was not found in the store.
    #[error("object not found: {0:?}")]
    ObjectNotFound(ObjectId),

    /// The object had an unexpected kind (e.g., expected tree, got blob).
    #[error("unexpected object kind for {id:?}: expected {expected}, got {actual}")]
    UnexpectedObjectKind {
        id: ObjectId,
        expected: String,
        actual: String,
    },

    /// Store operation failed.
    #[error("store error: {0}")]
    Store(#[from] wll_store::StoreError),

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Convenience alias for diff results.
pub type DiffResult<T> = Result<T, DiffError>;
