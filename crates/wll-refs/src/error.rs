//! Error types for reference operations.

use thiserror::Error;

/// Errors that can occur during reference operations.
#[derive(Debug, Error)]
pub enum RefError {
    /// The reference was not found.
    #[error("ref not found: {name}")]
    NotFound { name: String },

    /// A reference with this name already exists.
    #[error("ref already exists: {name}")]
    AlreadyExists { name: String },

    /// The branch name is invalid.
    #[error("invalid branch name: {name}: {reason}")]
    InvalidBranchName { name: String, reason: String },

    /// A tag is immutable and cannot be updated.
    #[error("tag is immutable: {name}")]
    TagImmutable { name: String },

    /// HEAD is in a detached state (not pointing to a branch).
    #[error("HEAD is detached")]
    DetachedHead,

    /// Cannot delete the currently checked-out branch.
    #[error("cannot delete current branch: {name}")]
    DeleteCurrentBranch { name: String },

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// I/O error during file-based ref operations.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience type alias for ref operations.
pub type Result<T> = std::result::Result<T, RefError>;
