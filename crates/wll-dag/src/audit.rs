//! Audit trail and impact analysis types.
//!
//! An [`AuditTrail`] captures the full causal chain leading to a specific
//! commitment, while an [`ImpactReport`] captures everything *downstream*
//! of a node (what was affected by it).

use serde::{Deserialize, Serialize};

use wll_types::{ObjectId, TemporalAnchor, WorldlineId};

use crate::node::CausalRelation;

/// A complete audit trail for a specific commitment.
///
/// The chain is ordered from the commitment itself backward through its
/// causal ancestors, following parent edges to reconstruct the full
/// provenance history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTrail {
    /// The commitment being audited.
    pub commitment: ObjectId,
    /// Ordered chain of audit entries (commitment first, then ancestors).
    pub chain: Vec<AuditEntry>,
}

impl AuditTrail {
    /// Create a new empty audit trail for the given commitment.
    pub fn new(commitment: ObjectId) -> Self {
        Self {
            commitment,
            chain: Vec::new(),
        }
    }

    /// Number of entries in the audit chain.
    pub fn len(&self) -> usize {
        self.chain.len()
    }

    /// Returns `true` if the audit trail is empty.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// Returns all unique worldlines involved in this audit trail.
    pub fn involved_worldlines(&self) -> Vec<WorldlineId> {
        let mut wls: Vec<WorldlineId> = self.chain.iter().map(|e| e.worldline.clone()).collect();
        wls.sort();
        wls.dedup();
        wls
    }
}

/// A single entry in an audit trail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// The node this entry describes.
    pub node: ObjectId,
    /// How this node relates to the next node in the chain.
    pub relation: CausalRelation,
    /// The worldline this node belongs to.
    pub worldline: WorldlineId,
    /// When this node was created.
    pub timestamp: TemporalAnchor,
    /// Human-readable summary of this entry.
    pub summary: String,
}

/// Impact analysis report for a node: what was affected downstream.
///
/// Given an origin node, the impact report describes how many worldlines,
/// receipts, and causal paths were affected by it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    /// The origin node being analyzed.
    pub origin: ObjectId,
    /// All worldlines that have at least one downstream receipt.
    pub affected_worldlines: Vec<WorldlineId>,
    /// Total number of downstream receipts (descendants).
    pub downstream_receipts: usize,
    /// Maximum depth of the cascade from the origin.
    pub cascade_depth: usize,
    /// Critical causal paths from origin to leaf descendants.
    pub critical_paths: Vec<Vec<ObjectId>>,
}

impl ImpactReport {
    /// Create an empty impact report for the given origin.
    pub fn new(origin: ObjectId) -> Self {
        Self {
            origin,
            affected_worldlines: Vec::new(),
            downstream_receipts: 0,
            cascade_depth: 0,
            critical_paths: Vec::new(),
        }
    }

    /// Returns `true` if there is no downstream impact.
    pub fn is_empty(&self) -> bool {
        self.downstream_receipts == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::identity::IdentityMaterial;

    fn test_worldline(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    #[test]
    fn empty_audit_trail() {
        let trail = AuditTrail::new(ObjectId::from_hash([1; 32]));
        assert!(trail.is_empty());
        assert_eq!(trail.len(), 0);
        assert!(trail.involved_worldlines().is_empty());
    }

    #[test]
    fn audit_trail_involved_worldlines_deduplicates() {
        let wl = test_worldline(1);
        let mut trail = AuditTrail::new(ObjectId::from_hash([1; 32]));
        trail.chain.push(AuditEntry {
            node: ObjectId::from_hash([2; 32]),
            relation: CausalRelation::Sequential,
            worldline: wl.clone(),
            timestamp: TemporalAnchor::new(1000, 0, 0),
            summary: "first".into(),
        });
        trail.chain.push(AuditEntry {
            node: ObjectId::from_hash([3; 32]),
            relation: CausalRelation::Sequential,
            worldline: wl.clone(),
            timestamp: TemporalAnchor::new(2000, 0, 0),
            summary: "second".into(),
        });
        let worldlines = trail.involved_worldlines();
        assert_eq!(worldlines.len(), 1);
    }

    #[test]
    fn empty_impact_report() {
        let report = ImpactReport::new(ObjectId::from_hash([1; 32]));
        assert!(report.is_empty());
        assert_eq!(report.downstream_receipts, 0);
        assert_eq!(report.cascade_depth, 0);
    }
}
