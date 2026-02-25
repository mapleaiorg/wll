//! DAG node types representing receipts in the causal provenance graph.
//!
//! Each [`DagNode`] corresponds to a receipt in a worldline stream and tracks
//! its causal parents via [`ParentRef`] edges. The [`CausalRelation`] enum
//! encodes the *kind* of causality (sequential, cross-worldline, evidence, etc.).

use serde::{Deserialize, Serialize};

use wll_types::{ObjectId, ReceiptKind, TemporalAnchor, WorldlineId};

/// A node in the provenance DAG.
///
/// Each node corresponds to a single receipt in a worldline stream. Nodes
/// are immutable once added to the DAG — they form an append-only structure
/// that can always be rebuilt from the underlying receipt streams.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNode {
    /// Content-addressed identifier for this node.
    pub id: ObjectId,
    /// The worldline this receipt belongs to.
    pub worldline: WorldlineId,
    /// Sequence number within the worldline stream.
    pub seq: u64,
    /// The kind of receipt (Commitment, Outcome, Snapshot).
    pub kind: ReceiptKind,
    /// Hybrid logical clock timestamp for causal ordering.
    pub timestamp: TemporalAnchor,
    /// Causal parent references (may be empty for root nodes).
    pub parents: Vec<ParentRef>,
    /// Additional metadata about this node.
    pub metadata: DagNodeMetadata,
}

impl DagNode {
    /// Returns `true` if this node has no parents (i.e., it is a root).
    pub fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    /// Returns the IDs of all parent nodes.
    pub fn parent_ids(&self) -> Vec<ObjectId> {
        self.parents.iter().map(|p| p.target).collect()
    }

    /// Returns parent references filtered by a specific relation type.
    pub fn parents_by_relation(&self, relation: CausalRelation) -> Vec<&ParentRef> {
        self.parents.iter().filter(|p| p.relation == relation).collect()
    }

    /// Returns a human-readable summary of this node.
    pub fn summary(&self) -> String {
        format!(
            "{} seq={} on {} ({})",
            self.kind,
            self.seq,
            self.worldline.short_id(),
            self.id.short_hex(),
        )
    }
}

/// A reference to a parent node with the type of causal relationship.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    /// The ObjectId of the parent node.
    pub target: ObjectId,
    /// The kind of causal relationship.
    pub relation: CausalRelation,
}

impl ParentRef {
    /// Create a new parent reference.
    pub fn new(target: ObjectId, relation: CausalRelation) -> Self {
        Self { target, relation }
    }

    /// Convenience constructor for a sequential parent.
    pub fn sequential(target: ObjectId) -> Self {
        Self::new(target, CausalRelation::Sequential)
    }

    /// Convenience constructor for a cross-worldline parent.
    pub fn cross_worldline(target: ObjectId) -> Self {
        Self::new(target, CausalRelation::CrossWorldline)
    }
}

/// The kind of causal relationship between two nodes in the DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CausalRelation {
    /// Previous receipt in same worldline stream (like a git parent commit).
    Sequential,
    /// This outcome was caused by that commitment.
    CommitmentToOutcome,
    /// This commitment references that commitment as evidence.
    EvidenceLink,
    /// Cross-worldline causal dependency.
    CrossWorldline,
    /// Merge: this receipt merges two worldline histories.
    Merge,
    /// Snapshot anchoring to a point-in-time state.
    SnapshotAnchor,
}

impl std::fmt::Display for CausalRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequential => write!(f, "Sequential"),
            Self::CommitmentToOutcome => write!(f, "CommitmentToOutcome"),
            Self::EvidenceLink => write!(f, "EvidenceLink"),
            Self::CrossWorldline => write!(f, "CrossWorldline"),
            Self::Merge => write!(f, "Merge"),
            Self::SnapshotAnchor => write!(f, "SnapshotAnchor"),
        }
    }
}

/// Optional metadata attached to a DAG node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNodeMetadata {
    /// Human-readable description of this node's purpose.
    pub description: Option<String>,
    /// Free-form tags for categorization.
    pub tags: Vec<String>,
    /// Content hash of the full receipt (for integrity verification).
    pub content_hash: Option<ObjectId>,
}

impl DagNodeMetadata {
    /// Create empty metadata.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create metadata with a description.
    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::identity::IdentityMaterial;

    fn test_worldline() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([1; 32]))
    }

    fn make_node(id_byte: u8, seq: u64, parents: Vec<ParentRef>) -> DagNode {
        DagNode {
            id: ObjectId::from_hash([id_byte; 32]),
            worldline: test_worldline(),
            seq,
            kind: ReceiptKind::Commitment,
            timestamp: TemporalAnchor::new(1000 + seq * 100, 0, 0),
            parents,
            metadata: DagNodeMetadata::empty(),
        }
    }

    #[test]
    fn root_node_has_no_parents() {
        let node = make_node(1, 0, vec![]);
        assert!(node.is_root());
        assert!(node.parent_ids().is_empty());
    }

    #[test]
    fn node_with_parents_is_not_root() {
        let parent_id = ObjectId::from_hash([0; 32]);
        let node = make_node(1, 1, vec![ParentRef::sequential(parent_id)]);
        assert!(!node.is_root());
        assert_eq!(node.parent_ids(), vec![parent_id]);
    }

    #[test]
    fn parents_by_relation_filters_correctly() {
        let seq_parent = ObjectId::from_hash([10; 32]);
        let cross_parent = ObjectId::from_hash([20; 32]);
        let node = make_node(
            1,
            2,
            vec![
                ParentRef::sequential(seq_parent),
                ParentRef::cross_worldline(cross_parent),
            ],
        );

        let sequential = node.parents_by_relation(CausalRelation::Sequential);
        assert_eq!(sequential.len(), 1);
        assert_eq!(sequential[0].target, seq_parent);

        let cross = node.parents_by_relation(CausalRelation::CrossWorldline);
        assert_eq!(cross.len(), 1);
        assert_eq!(cross[0].target, cross_parent);
    }

    #[test]
    fn summary_contains_key_info() {
        let node = make_node(1, 5, vec![]);
        let summary = node.summary();
        assert!(summary.contains("Commitment"));
        assert!(summary.contains("seq=5"));
    }

    #[test]
    fn causal_relation_display() {
        assert_eq!(format!("{}", CausalRelation::Sequential), "Sequential");
        assert_eq!(
            format!("{}", CausalRelation::CrossWorldline),
            "CrossWorldline"
        );
    }

    #[test]
    fn metadata_with_description() {
        let meta = DagNodeMetadata::with_description("test node");
        assert_eq!(meta.description.as_deref(), Some("test node"));
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let node = make_node(42, 7, vec![ParentRef::sequential(ObjectId::from_hash([0; 32]))]);
        let bytes = bincode::serialize(&node).unwrap();
        let deserialized: DagNode = bincode::deserialize(&bytes).unwrap();
        assert_eq!(node, deserialized);
    }
}
