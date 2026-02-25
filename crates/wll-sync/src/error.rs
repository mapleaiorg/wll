use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("remote error: {0}")]
    RemoteError(String),

    #[error("ref rejected: {name}: {reason}")]
    RefRejected { name: String, reason: String },

    #[error("receipt chain verification failed: {0}")]
    VerificationFailed(String),

    #[error("negotiation failed: {0}")]
    NegotiationFailed(String),

    #[error("transport error: {0}")]
    TransportError(String),

    #[error("not a fast-forward update for ref {0}")]
    NotFastForward(String),

    #[error("pack error: {0}")]
    Pack(#[from] wll_pack::PackError),

    #[error("ledger error: {0}")]
    Ledger(#[from] wll_ledger::LedgerError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SyncResult<T> = Result<T, SyncError>;
