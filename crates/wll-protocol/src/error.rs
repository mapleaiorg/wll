use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid message type: {0}")]
    InvalidMessageType(u8),

    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("framing error: {0}")]
    FramingError(String),

    #[error("version mismatch: local {local}, remote {remote}")]
    VersionMismatch { local: u32, remote: u32 },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("protocol error: code={code}, message={message}")]
    RemoteError { code: u32, message: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;
