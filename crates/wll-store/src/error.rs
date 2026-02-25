use wll_types::ObjectId;

/// Errors from object store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The requested object was not found.
    #[error("object not found: {0}")]
    NotFound(ObjectId),

    /// Content hash mismatch on read (data corruption).
    #[error("hash mismatch for {id}: expected {expected}, computed {computed}")]
    HashMismatch {
        id: ObjectId,
        expected: String,
        computed: String,
    },

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// I/O error from the underlying storage backend.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The object data is malformed or cannot be decoded.
    #[error("corrupt object {id}: {reason}")]
    CorruptObject { id: ObjectId, reason: String },

    /// Attempted to write a null object ID.
    #[error("cannot store object with null ID")]
    NullObjectId,

    /// Storage backend is read-only or otherwise unavailable.
    #[error("store is read-only")]
    ReadOnly,
}

/// Result alias for store operations.
pub type StoreResult<T> = Result<T, StoreError>;
