use std::fmt;

use serde::{Deserialize, Serialize};

use crate::temporal::TemporalAnchor;
use crate::identity::WorldlineId;

/// Unique identifier for a commitment (UUID v7 for time-ordering).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CommitmentId(uuid::Uuid);

impl CommitmentId {
    /// Generate a new time-ordered commitment ID (UUID v7).
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    /// Create from an existing UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// The underlying UUID.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }

    /// Short representation (first 8 characters of UUID).
    pub fn short_id(&self) -> String {
        self.0.to_string()[..8].to_string()
    }
}

impl Default for CommitmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CommitmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommitmentId({})", self.short_id())
    }
}

impl fmt::Display for CommitmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Classification of a commitment controlling policy tier.
///
/// The commitment class determines which policies are evaluated and what
/// level of evidence/signing is required.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommitmentClass {
    /// Read-only operation (lowest risk).
    ReadOnly,
    /// Content update (normal file changes).
    ContentUpdate,
    /// Structural change (directory reorganization, schema change).
    StructuralChange,
    /// Policy change (modifying governance rules).
    PolicyChange,
    /// Identity operation (key rotation, delegation).
    IdentityOperation,
    /// Custom class for domain-specific needs.
    Custom(String),
}

impl CommitmentClass {
    /// Risk level from 0 (lowest) to 4 (highest).
    pub fn risk_level(&self) -> u8 {
        match self {
            Self::ReadOnly => 0,
            Self::ContentUpdate => 1,
            Self::StructuralChange => 2,
            Self::PolicyChange => 3,
            Self::IdentityOperation => 4,
            Self::Custom(_) => 2, // default to medium
        }
    }
}

impl fmt::Display for CommitmentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::ContentUpdate => write!(f, "ContentUpdate"),
            Self::StructuralChange => write!(f, "StructuralChange"),
            Self::PolicyChange => write!(f, "PolicyChange"),
            Self::IdentityOperation => write!(f, "IdentityOperation"),
            Self::Custom(name) => write!(f, "Custom({name})"),
        }
    }
}

/// Policy evaluation result for a commitment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Commitment is accepted; proceed to outcome.
    Accepted,
    /// Commitment is rejected with reason.
    Rejected { reason: String },
    /// Decision is deferred; retry after specified time.
    Deferred {
        until: TemporalAnchor,
        reason: String,
    },
}

impl Decision {
    /// Returns `true` if accepted.
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// Returns `true` if rejected.
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }

    /// Returns `true` if deferred.
    pub fn is_deferred(&self) -> bool {
        matches!(self, Self::Deferred { .. })
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted => write!(f, "Accepted"),
            Self::Rejected { reason } => write!(f, "Rejected: {reason}"),
            Self::Deferred { reason, .. } => write!(f, "Deferred: {reason}"),
        }
    }
}

/// Reversibility classification for a commitment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reversibility {
    /// Fully reversible (can be undone without side effects).
    Reversible,
    /// Partially reversible (some effects cannot be undone).
    PartiallyReversible { constraints: String },
    /// Irreversible (cannot be undone once applied).
    Irreversible,
}

/// Unique identifier for a capability.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CapabilityId(pub String);

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Scope of a capability grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityScope {
    /// Global: applies everywhere.
    Global,
    /// Scoped to a specific worldline.
    Worldline(WorldlineId),
    /// Scoped to a file path pattern.
    Path(String),
    /// Custom scope for domain-specific needs.
    Custom(String),
}

/// A capability granted to a worldline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub id: CapabilityId,
    pub scope: CapabilityScope,
    pub granted_at: TemporalAnchor,
    pub expires_at: Option<TemporalAnchor>,
}

impl Capability {
    /// Returns `true` if the capability has expired at the given time.
    pub fn is_expired_at(&self, now: &TemporalAnchor) -> bool {
        self.expires_at
            .as_ref()
            .map(|exp| now.is_after(exp))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_id_is_unique() {
        let id1 = CommitmentId::new();
        let id2 = CommitmentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn commitment_id_short_format() {
        let id = CommitmentId::new();
        let short = id.short_id();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn commitment_class_risk_levels() {
        assert_eq!(CommitmentClass::ReadOnly.risk_level(), 0);
        assert_eq!(CommitmentClass::ContentUpdate.risk_level(), 1);
        assert_eq!(CommitmentClass::StructuralChange.risk_level(), 2);
        assert_eq!(CommitmentClass::PolicyChange.risk_level(), 3);
        assert_eq!(CommitmentClass::IdentityOperation.risk_level(), 4);
        assert_eq!(CommitmentClass::Custom("x".into()).risk_level(), 2);
    }

    #[test]
    fn decision_helpers() {
        assert!(Decision::Accepted.is_accepted());
        assert!(!Decision::Accepted.is_rejected());

        let rejected = Decision::Rejected {
            reason: "bad".into(),
        };
        assert!(rejected.is_rejected());
        assert!(!rejected.is_accepted());

        let deferred = Decision::Deferred {
            until: TemporalAnchor::zero(),
            reason: "wait".into(),
        };
        assert!(deferred.is_deferred());
    }

    #[test]
    fn reversibility_variants() {
        let rev = Reversibility::Reversible;
        let partial = Reversibility::PartiallyReversible {
            constraints: "rollback may lose cache".into(),
        };
        let irrev = Reversibility::Irreversible;
        // Ensure all variants serialize
        let _ = serde_json::to_string(&rev).unwrap();
        let _ = serde_json::to_string(&partial).unwrap();
        let _ = serde_json::to_string(&irrev).unwrap();
    }

    #[test]
    fn capability_expiry() {
        let cap = Capability {
            id: CapabilityId("write".into()),
            scope: CapabilityScope::Global,
            granted_at: TemporalAnchor::new(100, 0, 0),
            expires_at: Some(TemporalAnchor::new(200, 0, 0)),
        };

        assert!(!cap.is_expired_at(&TemporalAnchor::new(150, 0, 0)));
        assert!(cap.is_expired_at(&TemporalAnchor::new(250, 0, 0)));
    }

    #[test]
    fn capability_no_expiry() {
        let cap = Capability {
            id: CapabilityId("admin".into()),
            scope: CapabilityScope::Global,
            granted_at: TemporalAnchor::zero(),
            expires_at: None,
        };
        assert!(!cap.is_expired_at(&TemporalAnchor::new(u64::MAX, u32::MAX, u16::MAX)));
    }

    #[test]
    fn serde_roundtrip() {
        let class = CommitmentClass::Custom("deploy".into());
        let json = serde_json::to_string(&class).unwrap();
        let parsed: CommitmentClass = serde_json::from_str(&json).unwrap();
        assert_eq!(class, parsed);

        let decision = Decision::Rejected {
            reason: "policy denied".into(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: Decision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, parsed);
    }
}
