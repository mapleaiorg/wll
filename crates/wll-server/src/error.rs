use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("repository not found: {0}")]
    RepoNotFound(String),

    #[error("repository already exists: {0}")]
    RepoAlreadyExists(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("authorization denied: {action} on {repo}")]
    AuthorizationDenied { repo: String, action: String },

    #[error("protocol error: {0}")]
    Protocol(#[from] wll_protocol::ProtocolError),

    #[error("store error: {0}")]
    Store(#[from] wll_store::StoreError),

    #[error("ledger error: {0}")]
    Ledger(#[from] wll_ledger::LedgerError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type ServerResult<T> = Result<T, ServerError>;
