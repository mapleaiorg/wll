//! Causal provenance DAG for the WorldLine Ledger.
//!
//! Tracks causal relationships between receipts across worldlines. Supports
//! traversal queries (ancestors, descendants, paths), audit trails, impact
//! analysis, and topological ordering.

pub mod audit;
pub mod dag;
pub mod error;
pub mod node;

pub use audit::{AuditEntry, AuditTrail, ImpactReport};
pub use dag::ProvenanceDag;
pub use error::{DagError, DagResult};
pub use node::{CausalRelation, DagNode, DagNodeMetadata, ParentRef};
