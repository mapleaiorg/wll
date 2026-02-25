use thiserror::Error;
use wll_types::ObjectId;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("invalid pack magic: expected {expected}, got {actual}")]
    InvalidMagic { expected: String, actual: String },

    #[error("unsupported pack version: {0}")]
    UnsupportedVersion(u32),

    #[error("pack checksum mismatch")]
    ChecksumMismatch,

    #[error("object not found in pack: {0}")]
    ObjectNotFound(ObjectId),

    #[error("corrupt pack entry at offset {offset}: {reason}")]
    CorruptEntry { offset: u64, reason: String },

    #[error("CRC32 mismatch for object {id}")]
    CrcMismatch { id: ObjectId },

    #[error("decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("compression failed: {0}")]
    CompressionFailed(String),

    #[error("delta base not found: {0}")]
    DeltaBaseNotFound(ObjectId),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("index corrupted: {0}")]
    IndexCorrupted(String),
}

pub type PackResult<T> = Result<T, PackError>;
