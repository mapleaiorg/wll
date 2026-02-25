use std::io;
use std::path::PathBuf;

/// Errors produced by the event fabric subsystem.
#[derive(Debug, thiserror::Error)]
pub enum FabricError {
    /// I/O error during WAL or file operations.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// CRC integrity check failed for a WAL entry.
    #[error("CRC integrity check failed at offset {offset}: expected {expected:#010x}, got {actual:#010x}")]
    CrcMismatch {
        offset: u64,
        expected: u32,
        actual: u32,
    },

    /// WAL entry has an invalid length field.
    #[error("invalid WAL entry length {length} at offset {offset}")]
    InvalidEntryLength { offset: u64, length: u32 },

    /// WAL segment file not found or inaccessible.
    #[error("WAL path not found: {0}")]
    WalPathNotFound(PathBuf),

    /// The fabric has been shut down and cannot accept events.
    #[error("fabric is shut down")]
    Shutdown,

    /// A subscriber channel is closed.
    #[error("subscriber channel closed")]
    SubscriberClosed,

    /// Event router has no subscribers for the given filter.
    #[error("no subscribers matched the event filter")]
    NoSubscribers,

    /// HLC clock drift exceeds the allowed threshold.
    #[error("clock drift too large: local={local_ms}ms, received={received_ms}ms, max_drift={max_drift_ms}ms")]
    ClockDrift {
        local_ms: u64,
        received_ms: u64,
        max_drift_ms: u64,
    },

    /// Checkpoint offset is beyond the current WAL write position.
    #[error("checkpoint offset {requested} exceeds current write position {current}")]
    InvalidCheckpoint { requested: u64, current: u64 },
}

/// Convenience alias used throughout the fabric crate.
pub type Result<T> = std::result::Result<T, FabricError>;
