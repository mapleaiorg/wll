use std::fmt;

use serde::{Deserialize, Serialize};

use crate::identity::WorldlineId;

/// Unique identifier for a receipt within a worldline stream.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptId {
    /// The worldline this receipt belongs to.
    pub worldline: WorldlineId,
    /// Sequence number within the stream (1-based, monotonic).
    pub seq: u64,
    /// BLAKE3 hash of the receipt content.
    pub hash: [u8; 32],
}

impl ReceiptId {
    /// Create a new receipt ID.
    pub fn new(worldline: WorldlineId, seq: u64, hash: [u8; 32]) -> Self {
        Self {
            worldline,
            seq,
            hash,
        }
    }

    /// Short hex representation of the hash.
    pub fn short_hash(&self) -> String {
        hex::encode(&self.hash[..4])
    }
}

impl fmt::Display for ReceiptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r#{} [{}]", self.seq, self.short_hash())
    }
}

/// Kind of receipt in a worldline stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReceiptKind {
    /// Commitment: a proposal was evaluated by the gate.
    Commitment,
    /// Outcome: the effects of an accepted (or rejected) commitment.
    Outcome,
    /// Snapshot: a point-in-time state checkpoint.
    Snapshot,
}

impl fmt::Display for ReceiptKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commitment => write!(f, "Commitment"),
            Self::Outcome => write!(f, "Outcome"),
            Self::Snapshot => write!(f, "Snapshot"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityMaterial;

    #[test]
    fn receipt_id_display() {
        let wid = WorldlineId::derive(&IdentityMaterial::GenesisHash([1; 32]));
        let rid = ReceiptId::new(wid, 42, [0xab; 32]);
        let display = format!("{rid}");
        assert!(display.contains("r#42"));
        assert!(display.contains("abababab"));
    }

    #[test]
    fn receipt_kind_display() {
        assert_eq!(format!("{}", ReceiptKind::Commitment), "Commitment");
        assert_eq!(format!("{}", ReceiptKind::Outcome), "Outcome");
        assert_eq!(format!("{}", ReceiptKind::Snapshot), "Snapshot");
    }

    #[test]
    fn serde_roundtrip() {
        let kind = ReceiptKind::Commitment;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: ReceiptKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }
}
