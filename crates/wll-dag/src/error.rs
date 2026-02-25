//! Error types for the provenance DAG.

use wll_types::ObjectId;

/// Errors that can occur during DAG operations.
#[derive(Debug, thiserror::Error)]
pub enum DagError {
    /// A referenced node was not found in the DAG.
    #[error("node not found: {0:?}")]
    NodeNotFound(ObjectId),

    /// A parent reference points to a node that does not exist.
    #[error("dangling parent reference: node {node:?} references missing parent {parent:?}")]
    DanglingParent {
        /// The node containing the bad reference.
        node: ObjectId,
        /// The missing parent.
        parent: ObjectId,
    },

    /// Attempted to add a node with an ID that already exists.
    #[error("duplicate node: {0:?}")]
    DuplicateNode(ObjectId),

    /// A cycle was detected, which violates the DAG invariant.
    #[error("cycle detected involving node {0:?}")]
    CycleDetected(ObjectId),

    /// Temporal ordering violation: a child has a timestamp before its parent.
    #[error("temporal ordering violation: child {child:?} is before parent {parent:?}")]
    TemporalViolation {
        /// The child node.
        child: ObjectId,
        /// The parent node whose timestamp is later.
        parent: ObjectId,
    },

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Storage I/O error.
    #[error("storage error: {0}")]
    Storage(String),
}

/// Convenience alias for DAG results.
pub type DagResult<T> = Result<T, DagError>;
