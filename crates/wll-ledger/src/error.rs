/// Errors produced by ledger operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    #[error("integrity violation at seq {seq}: {reason}")]
    IntegrityViolation { seq: u64, reason: String },

    #[error("commitment receipt not found for the given hash")]
    MissingCommitmentReceipt,

    #[error("commitment was not accepted; cannot append accepted outcome")]
    CommitmentNotAccepted,

    #[error("commitment was not rejected; cannot append rejection outcome")]
    CommitmentNotRejected,

    #[error("snapshot anchor receipt not found in stream")]
    MissingSnapshotAnchor,

    #[error("hash collision detected")]
    HashCollision,

    #[error("invalid sequence range: from={from}, to={to}")]
    InvalidRange { from: u64, to: u64 },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("worldline not found")]
    WorldlineNotFound,

    #[error("store error: {0}")]
    StoreError(String),
}
