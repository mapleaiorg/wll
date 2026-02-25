use serde::{Deserialize, Serialize};

use wll_types::{CommitmentId, ObjectId, ReceiptKind, TemporalAnchor, WorldlineId};

/// Unique identifier for a fabric event.
///
/// Composed of the timestamp plus a BLAKE3 hash of the event content,
/// making it both time-ordered and content-addressable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId {
    /// Content hash of the event (BLAKE3).
    pub hash: [u8; 32],
}

impl EventId {
    /// Create an `EventId` from a raw hash.
    pub fn from_hash(hash: [u8; 32]) -> Self {
        Self { hash }
    }

    /// Short hex representation (first 8 hex chars).
    pub fn short_hex(&self) -> String {
        hex::encode(&self.hash[..4])
    }

    /// Full hex representation.
    pub fn to_hex(&self) -> String {
        hex::encode(self.hash)
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evt:{}", self.short_hex())
    }
}

/// Classification of fabric events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    /// A new commitment has been proposed.
    CommitmentProposed,
    /// A commitment decision has been reached.
    CommitmentDecided,
    /// An outcome has been recorded in the ledger.
    OutcomeRecorded,
    /// A snapshot checkpoint has been created.
    SnapshotCreated,
    /// A new worldline has been created.
    WorldlineCreated,
    /// A reference (branch/tag) has been updated.
    RefUpdated,
    /// A sync operation has started.
    SyncStarted,
    /// A sync operation has completed.
    SyncCompleted,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CommitmentProposed => "CommitmentProposed",
            Self::CommitmentDecided => "CommitmentDecided",
            Self::OutcomeRecorded => "OutcomeRecorded",
            Self::SnapshotCreated => "SnapshotCreated",
            Self::WorldlineCreated => "WorldlineCreated",
            Self::RefUpdated => "RefUpdated",
            Self::SyncStarted => "SyncStarted",
            Self::SyncCompleted => "SyncCompleted",
        };
        write!(f, "{s}")
    }
}

/// Payload data carried by a fabric event.
///
/// Different event kinds carry different payload shapes. The payload is
/// always serializable for WAL persistence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventPayload {
    /// Empty payload (event kind is self-describing).
    Empty,
    /// Commitment-related payload.
    Commitment {
        commitment_id: CommitmentId,
        description: String,
    },
    /// Object reference payload.
    ObjectRef {
        object_id: ObjectId,
        receipt_kind: ReceiptKind,
    },
    /// Sync payload with remote node info.
    Sync {
        remote_node: String,
        objects_transferred: u64,
    },
    /// Ref update payload.
    RefUpdate {
        ref_name: String,
        old_target: Option<ObjectId>,
        new_target: ObjectId,
    },
    /// Arbitrary binary data.
    Raw(Vec<u8>),
}

/// A single event flowing through the fabric.
///
/// Every event carries a unique ID, an HLC timestamp, the worldline it
/// belongs to, a classification kind, a payload, and a BLAKE3 integrity hash
/// computed over the serialized (kind + worldline + payload + timestamp).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FabricEvent {
    /// Unique event identifier (content-addressed).
    pub id: EventId,
    /// HLC timestamp when the event was created.
    pub timestamp: TemporalAnchor,
    /// The worldline this event pertains to.
    pub worldline: WorldlineId,
    /// Classification of this event.
    pub kind: EventKind,
    /// Event-specific payload data.
    pub payload: EventPayload,
    /// BLAKE3 integrity hash over (kind, worldline, payload, timestamp).
    pub integrity_hash: [u8; 32],
}

impl FabricEvent {
    /// Build a new `FabricEvent`, computing its integrity hash and event ID.
    pub fn new(
        timestamp: TemporalAnchor,
        worldline: WorldlineId,
        kind: EventKind,
        payload: EventPayload,
    ) -> Self {
        let integrity_hash = Self::compute_integrity(&timestamp, &worldline, &kind, &payload);
        let id = EventId::from_hash(integrity_hash);
        Self {
            id,
            timestamp,
            worldline,
            kind,
            payload,
            integrity_hash,
        }
    }

    /// Verify the event's integrity hash matches its content.
    pub fn verify_integrity(&self) -> bool {
        let expected =
            Self::compute_integrity(&self.timestamp, &self.worldline, &self.kind, &self.payload);
        self.integrity_hash == expected
    }

    /// Compute the BLAKE3 integrity hash over the event's core fields.
    fn compute_integrity(
        timestamp: &TemporalAnchor,
        worldline: &WorldlineId,
        kind: &EventKind,
        payload: &EventPayload,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"wll-fabric-event-v1:");

        // Hash the timestamp fields
        hasher.update(&timestamp.physical_ms.to_le_bytes());
        hasher.update(&timestamp.logical.to_le_bytes());
        hasher.update(&timestamp.node_id.to_le_bytes());

        // Hash the worldline
        hasher.update(worldline.as_bytes());

        // Hash the kind via bincode
        if let Ok(kind_bytes) = bincode::serialize(kind) {
            hasher.update(&kind_bytes);
        }

        // Hash the payload via bincode
        if let Ok(payload_bytes) = bincode::serialize(payload) {
            hasher.update(&payload_bytes);
        }

        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::IdentityMaterial;

    fn test_worldline() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([42u8; 32]))
    }

    #[test]
    fn event_integrity_roundtrip() {
        let event = FabricEvent::new(
            TemporalAnchor::new(1000, 0, 1),
            test_worldline(),
            EventKind::CommitmentProposed,
            EventPayload::Empty,
        );
        assert!(event.verify_integrity());
    }

    #[test]
    fn event_id_is_deterministic() {
        let ts = TemporalAnchor::new(500, 3, 1);
        let wl = test_worldline();
        let e1 = FabricEvent::new(ts, wl.clone(), EventKind::RefUpdated, EventPayload::Empty);
        let e2 = FabricEvent::new(ts, wl, EventKind::RefUpdated, EventPayload::Empty);
        assert_eq!(e1.id, e2.id);
    }

    #[test]
    fn different_kinds_produce_different_ids() {
        let ts = TemporalAnchor::new(500, 0, 1);
        let wl = test_worldline();
        let e1 = FabricEvent::new(
            ts,
            wl.clone(),
            EventKind::CommitmentProposed,
            EventPayload::Empty,
        );
        let e2 = FabricEvent::new(ts, wl, EventKind::OutcomeRecorded, EventPayload::Empty);
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn event_kind_display() {
        assert_eq!(format!("{}", EventKind::CommitmentProposed), "CommitmentProposed");
        assert_eq!(format!("{}", EventKind::SyncCompleted), "SyncCompleted");
    }

    #[test]
    fn event_id_display() {
        let id = EventId::from_hash([0xab; 32]);
        let display = format!("{id}");
        assert!(display.starts_with("evt:"));
        assert_eq!(display, "evt:abababab");
    }

    #[test]
    fn serde_roundtrip() {
        let event = FabricEvent::new(
            TemporalAnchor::new(1000, 0, 1),
            test_worldline(),
            EventKind::WorldlineCreated,
            EventPayload::Raw(vec![1, 2, 3]),
        );
        let bytes = bincode::serialize(&event).unwrap();
        let decoded: FabricEvent = bincode::deserialize(&bytes).unwrap();
        assert_eq!(event, decoded);
        assert!(decoded.verify_integrity());
    }
}
