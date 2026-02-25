use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("repository not initialized at {0}")]
    NotInitialized(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("object not found: {0}")]
    ObjectNotFound(String),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error("commitment rejected: {0}")]
    CommitmentRejected(String),

    #[error("store error: {0}")]
    Store(#[from] wll_store::StoreError),

    #[error("ledger error: {0}")]
    Ledger(#[from] wll_ledger::LedgerError),

    #[error("ref error: {0}")]
    Ref(#[from] wll_refs::RefError),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type SdkResult<T> = Result<T, SdkError>;
