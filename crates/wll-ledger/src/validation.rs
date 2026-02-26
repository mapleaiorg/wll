use std::collections::HashSet;

use wll_types::WorldlineId;

use crate::error::LedgerError;
use crate::records::Receipt;
use crate::traits::LedgerReader;

/// Result of stream validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationReport {
    pub worldline: WorldlineId,
    pub receipt_count: u64,
    pub hash_chain_valid: bool,
    pub sequence_monotonic: bool,
    pub outcomes_attributed: bool,
    pub snapshots_anchored: bool,
    pub violations: Vec<Violation>,
}

impl ValidationReport {
    /// Returns `true` if all checks passed.
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }
}

/// A specific integrity violation detected during validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub seq: u64,
    pub kind: ViolationKind,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViolationKind {
    SequenceGap,
    HashChainBreak,
    HashMismatch,
    UnattributedOutcome,
    UnanchoredSnapshot,
}

/// Stream integrity validator.
pub struct StreamValidator;

impl StreamValidator {
    /// Validate a single worldline stream for all invariants.
    pub fn validate_stream<R: LedgerReader>(
        reader: &R,
        worldline: &WorldlineId,
    ) -> Result<ValidationReport, LedgerError> {
        let receipts = reader.read_all(worldline)?;
        let mut violations = Vec::new();
        let mut hash_chain_valid = true;
        let mut sequence_monotonic = true;
        let mut outcomes_attributed = true;
        let mut snapshots_anchored = true;
        let mut seen_hashes = HashSet::new();
        let mut commitment_hashes = HashSet::new();

        for (index, receipt) in receipts.iter().enumerate() {
            let expected_seq = (index + 1) as u64;
            if receipt.seq() != expected_seq {
                sequence_monotonic = false;
                violations.push(Violation {
                    seq: receipt.seq(),
                    kind: ViolationKind::SequenceGap,
                    description: format!(
                        "expected seq {expected_seq}, got {}",
                        receipt.seq()
                    ),
                });
            }

            // Check prev_hash link
            let expected_prev = if index == 0 {
                None
            } else {
                Some(receipts[index - 1].receipt_hash())
            };
            if receipt.prev_hash() != expected_prev {
                hash_chain_valid = false;
                violations.push(Violation {
                    seq: receipt.seq(),
                    kind: ViolationKind::HashChainBreak,
                    description: "previous hash link mismatch".into(),
                });
            }

            // Recompute and verify hash
            let computed = recompute_hash(receipt);
            if let Ok(h) = computed {
                if h != receipt.receipt_hash() {
                    hash_chain_valid = false;
                    violations.push(Violation {
                        seq: receipt.seq(),
                        kind: ViolationKind::HashMismatch,
                        description: "receipt hash does not match computed".into(),
                    });
                }
            }

            seen_hashes.insert(receipt.receipt_hash());

            // Type-specific checks
            match receipt {
                Receipt::Commitment(c) => {
                    commitment_hashes.insert(c.receipt_hash);
                }
                Receipt::Outcome(o) => {
                    if !commitment_hashes.contains(&o.commitment_receipt_hash) {
                        outcomes_attributed = false;
                        violations.push(Violation {
                            seq: receipt.seq(),
                            kind: ViolationKind::UnattributedOutcome,
                            description: "outcome references missing commitment".into(),
                        });
                    }
                }
                Receipt::Snapshot(s) => {
                    if !seen_hashes.contains(&s.anchored_receipt_hash) {
                        snapshots_anchored = false;
                        violations.push(Violation {
                            seq: receipt.seq(),
                            kind: ViolationKind::UnanchoredSnapshot,
                            description: "snapshot anchor missing in stream".into(),
                        });
                    }
                }
            }
        }

        Ok(ValidationReport {
            worldline: worldline.clone(),
            receipt_count: receipts.len() as u64,
            hash_chain_valid,
            sequence_monotonic,
            outcomes_attributed,
            snapshots_anchored,
            violations,
        })
    }

    /// Validate all worldlines in the ledger.
    pub fn validate_all<R: LedgerReader>(
        reader: &R,
    ) -> Result<Vec<ValidationReport>, LedgerError> {
        let worldlines = reader.worldlines()?;
        let mut reports = Vec::new();
        for wid in &worldlines {
            reports.push(Self::validate_stream(reader, wid)?);
        }
        Ok(reports)
    }
}

fn recompute_hash(receipt: &Receipt) -> Result<[u8; 32], LedgerError> {
    let mut canonical = receipt.clone();
    canonical.set_receipt_hash([0; 32]);
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|e| LedgerError::Serialization(e.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"wll-receipt-v1:");
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
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
            intent: "validate test".into(),
            requested_caps: vec![],
            targets: vec![worldline.clone()],
            evidence: EvidenceBundle::empty(),
            nonce: 1,
        }
    }

    #[test]
    fn valid_stream_passes() {
        let ledger = InMemoryLedger::default();
        let wid = worldline(1);

        let c = ledger
            .append_commitment(&proposal(&wid), &Decision::Accepted, [1; 32])
            .unwrap();
        ledger
            .append_outcome(
                c.receipt_hash,
                &OutcomeRecord {
                    effects: vec![],
                    proofs: vec![],
                    state_updates: vec![StateUpdate {
                        key: "k".into(),
                        value: Value::from(1),
                    }],
                    metadata: BTreeMap::new(),
                },
            )
            .unwrap();

        let report = StreamValidator::validate_stream(&ledger, &wid).unwrap();
        assert!(report.is_valid());
        assert_eq!(report.receipt_count, 2);
    }

    #[test]
    fn validate_all_checks_multiple_worldlines() {
        let ledger = InMemoryLedger::default();
        let wid1 = worldline(10);
        let wid2 = worldline(20);

        ledger
            .append_commitment(&proposal(&wid1), &Decision::Accepted, [1; 32])
            .unwrap();
        ledger
            .append_commitment(&proposal(&wid2), &Decision::Accepted, [1; 32])
            .unwrap();

        let reports = StreamValidator::validate_all(&ledger).unwrap();
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().all(|r| r.is_valid()));
    }

    #[test]
    fn empty_worldline_is_valid() {
        let ledger = InMemoryLedger::default();
        let wid = worldline(99);
        let report = StreamValidator::validate_stream(&ledger, &wid).unwrap();
        assert!(report.is_valid());
        assert_eq!(report.receipt_count, 0);
    }
}
