use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use wll_types::{CommitmentId, TemporalAnchor, WorldlineId};

use crate::error::LedgerError;
use crate::records::{Receipt, ReceiptKind, ReceiptRef};
use crate::traits::LedgerReader;

/// Latest worldline state reconstructed from receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestStateProjection {
    pub worldline: WorldlineId,
    pub head: Option<ReceiptRef>,
    pub latest_commitment: Option<CommitmentId>,
    pub trajectory_length: u64,
    pub last_updated: Option<TemporalAnchor>,
    pub state: BTreeMap<String, Value>,
}

/// Row in the audit index for compliance/audit workflows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditIndexEntry {
    pub seq: u64,
    pub receipt_hash: [u8; 32],
    pub kind: ReceiptKind,
    pub timestamp: TemporalAnchor,
    pub commitment_id: Option<CommitmentId>,
    pub accepted: Option<bool>,
    pub summary: String,
}

/// Immutable sequence of receipt summaries for audit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditIndexProjection {
    pub worldline: WorldlineId,
    pub entries: Vec<AuditIndexEntry>,
}

/// Deterministic projection builders.
pub struct ProjectionBuilder;

impl ProjectionBuilder {
    pub fn latest_state<R: LedgerReader>(
        reader: &R,
        worldline: &WorldlineId,
    ) -> Result<LatestStateProjection, LedgerError> {
        let receipts = reader.read_all(worldline)?;
        let mut state = BTreeMap::new();
        let mut latest_commitment = None;
        let mut last_updated = None;

        for receipt in &receipts {
            match receipt {
                Receipt::Commitment(c) => {
                    latest_commitment = Some(c.commitment_id.clone());
                }
                Receipt::Outcome(o) => {
                    if o.accepted {
                        for update in &o.state_updates {
                            state.insert(update.key.clone(), update.value.clone());
                        }
                    }
                }
                Receipt::Snapshot(s) => {
                    state = s.state.clone();
                }
            }
            last_updated = Some(receipt.timestamp());
        }

        Ok(LatestStateProjection {
            worldline: worldline.clone(),
            head: receipts.last().map(ReceiptRef::from),
            latest_commitment,
            trajectory_length: receipts.len() as u64,
            last_updated,
            state,
        })
    }

    pub fn audit_index<R: LedgerReader>(
        reader: &R,
        worldline: &WorldlineId,
    ) -> Result<AuditIndexProjection, LedgerError> {
        let receipts = reader.read_all(worldline)?;
        let mut commitment_by_hash = HashMap::new();

        for receipt in &receipts {
            if let Receipt::Commitment(c) = receipt {
                commitment_by_hash.insert(c.receipt_hash, c.commitment_id.clone());
            }
        }

        let entries = receipts
            .iter()
            .map(|receipt| match receipt {
                Receipt::Commitment(c) => AuditIndexEntry {
                    seq: c.seq,
                    receipt_hash: c.receipt_hash,
                    kind: ReceiptKind::Commitment,
                    timestamp: c.timestamp,
                    commitment_id: Some(c.commitment_id.clone()),
                    accepted: Some(c.decision.is_accepted()),
                    summary: c.intent.clone(),
                },
                Receipt::Outcome(o) => AuditIndexEntry {
                    seq: o.seq,
                    receipt_hash: o.receipt_hash,
                    kind: ReceiptKind::Outcome,
                    timestamp: o.timestamp,
                    commitment_id: commitment_by_hash
                        .get(&o.commitment_receipt_hash)
                        .cloned(),
                    accepted: Some(o.accepted),
                    summary: if o.accepted {
                        format!(
                            "{} effect(s), {} proof(s)",
                            o.effects.len(),
                            o.proofs.len()
                        )
                    } else {
                        "rejected outcome".into()
                    },
                },
                Receipt::Snapshot(s) => AuditIndexEntry {
                    seq: s.seq,
                    receipt_hash: s.receipt_hash,
                    kind: ReceiptKind::Snapshot,
                    timestamp: s.timestamp,
                    commitment_id: None,
                    accepted: None,
                    summary: format!(
                        "snapshot anchored at {}",
                        short_hash(s.anchored_receipt_hash)
                    ),
                },
            })
            .collect();

        Ok(AuditIndexProjection {
            worldline: worldline.clone(),
            entries,
        })
    }
}

fn short_hash(hash: [u8; 32]) -> String {
    hash[..6].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;
    use wll_types::{CommitmentId, identity::IdentityMaterial};

    use crate::memory::InMemoryLedger;
    use crate::records::*;
    use crate::traits::LedgerWriter;

    use super::*;

    fn worldline(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    fn proposal(worldline: &WorldlineId) -> CommitmentProposal {
        CommitmentProposal {
            worldline: worldline.clone(),
            commitment_id: CommitmentId::new(),
            class: wll_types::CommitmentClass::ContentUpdate,
            intent: "projection test".into(),
            requested_caps: vec!["cap-test".into()],
            targets: vec![worldline.clone()],
            evidence: EvidenceBundle::from_references(vec!["obj://proof".into()]),
            nonce: 9,
        }
    }

    fn outcome(key: &str, value: i64) -> OutcomeRecord {
        OutcomeRecord {
            effects: vec![],
            proofs: vec![],
            state_updates: vec![StateUpdate {
                key: key.to_string(),
                value: Value::from(value),
            }],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn latest_state_projection_is_deterministic() {
        let ledger = InMemoryLedger::default();
        let wid = worldline(1);

        let c = ledger
            .append_commitment(&proposal(&wid), &Decision::Accepted, [1; 32])
            .unwrap();
        let o = ledger
            .append_outcome(c.receipt_hash, &outcome("balance", 40))
            .unwrap();

        let mut snap_state = BTreeMap::new();
        snap_state.insert("balance".into(), Value::from(40));
        ledger
            .append_snapshot(&SnapshotInput {
                worldline: wid.clone(),
                anchored_receipt_hash: o.receipt_hash,
                state: snap_state,
            })
            .unwrap();

        let first = ProjectionBuilder::latest_state(&ledger, &wid).unwrap();
        let second = ProjectionBuilder::latest_state(&ledger, &wid).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.state.get("balance"), Some(&Value::from(40)));
    }

    #[test]
    fn audit_index_contains_all_receipts() {
        let ledger = InMemoryLedger::default();
        let wid = worldline(2);

        let c = ledger
            .append_commitment(&proposal(&wid), &Decision::Accepted, [2; 32])
            .unwrap();
        ledger
            .append_outcome(c.receipt_hash, &outcome("x", 1))
            .unwrap();

        let projection = ProjectionBuilder::audit_index(&ledger, &wid).unwrap();
        assert_eq!(projection.entries.len(), 2);
        assert_eq!(projection.entries[0].kind, ReceiptKind::Commitment);
        assert_eq!(projection.entries[1].kind, ReceiptKind::Outcome);
    }
}
