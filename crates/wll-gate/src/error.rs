use std::fmt;

/// Errors that can occur during gate evaluation.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    /// A required field is missing or empty in the proposal.
    #[error("validation error: {0}")]
    Validation(String),

    /// The proposer lacks a required capability.
    #[error("capability denied: {0}")]
    CapabilityDenied(String),

    /// A policy rule was violated.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// The gate evaluation timed out.
    #[error("gate evaluation timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// A stage returned an unexpected error.
    #[error("stage error in '{stage}': {message}")]
    StageError { stage: String, message: String },

    /// Configuration is invalid.
    #[error("configuration error: {0}")]
    Config(String),
}

impl GateError {
    /// Create a stage error with a name and message.
    pub fn stage(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::StageError {
            stage: stage.into(),
            message: message.into(),
        }
    }
}

impl PartialEq for GateError {
    fn eq(&self, other: &Self) -> bool {
        // Compare by display representation for test convenience.
        fmt::format(format_args!("{self}")) == fmt::format(format_args!("{other}"))
    }
}

impl Eq for GateError {}
