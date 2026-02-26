use wll_ledger::Receipt;
use wll_types::WorldlineId;

use crate::error::SyncResult;
use crate::types::VerificationReport;

/// Verifies receipt chain integrity after receiving from a remote.
pub struct SyncVerifier;

impl SyncVerifier {
    /// Verify a batch of received receipts for a worldline.
    pub fn verify_received_receipts(
        receipts: &[Receipt],
        worldline: &WorldlineId,
    ) -> SyncResult<VerificationReport> {
        let mut violations = Vec::new();
        let mut prev_hash: Option<[u8; 32]> = None;
        let mut prev_seq: Option<u64> = None;

        for receipt in receipts {
            if receipt.worldline() != worldline {
                violations.push(format!("seq {}: wrong worldline", receipt.seq()));
            }

            if let Some(ps) = prev_seq {
                if receipt.seq() != ps + 1 {
                    violations.push(format!("seq {}: expected {}, gap", receipt.seq(), ps + 1));
                }
            }

            if receipt.prev_hash() != prev_hash {
                violations.push(format!("seq {}: prev_hash mismatch", receipt.seq()));
            }

            prev_hash = Some(receipt.receipt_hash());
            prev_seq = Some(receipt.seq());
        }

        Ok(VerificationReport {
            worldline: worldline.clone(),
            receipts_verified: receipts.len() as u64,
            chain_valid: violations.is_empty(),
            violations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use wll_ledger::CommitmentReceipt;
    use wll_types::{CommitmentClass, CommitmentId, TemporalAnchor};
    use wll_types::commitment::Decision;
    use wll_types::evidence::EvidenceBundle;
    use wll_types::identity::IdentityMaterial;

    fn wl(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    fn make_commitment(worldline: &WorldlineId, seq: u64, prev: Option<[u8; 32]>, hash: [u8; 32]) -> Receipt {
        Receipt::Commitment(CommitmentReceipt {
            worldline: worldline.clone(),
            seq,
            receipt_hash: hash,
            prev_hash: prev,
            timestamp: TemporalAnchor::new(seq * 1000, 0, 0),
            proposal_hash: [0; 32],
            commitment_id: CommitmentId::new(),
            class: CommitmentClass::ContentUpdate,
            intent: "test".into(),
            requested_caps: vec![],
            evidence: EvidenceBundle::empty(),
            decision: Decision::Accepted,
            policy_hash: [0; 32],
        })
    }

    #[test]
    fn valid_chain_passes() {
        let w = wl(1);
        let r1 = make_commitment(&w, 1, None, [1; 32]);
        let r2 = make_commitment(&w, 2, Some([1; 32]), [2; 32]);
        let report = SyncVerifier::verify_received_receipts(&[r1, r2], &w).unwrap();
        assert!(report.chain_valid);
        assert_eq!(report.receipts_verified, 2);
    }

    #[test]
    fn sequence_gap_detected() {
        let w = wl(2);
        let r1 = make_commitment(&w, 1, None, [1; 32]);
        let r3 = make_commitment(&w, 3, Some([1; 32]), [3; 32]); // gap: missing 2
        let report = SyncVerifier::verify_received_receipts(&[r1, r3], &w).unwrap();
        assert!(!report.chain_valid);
        assert!(report.violations.iter().any(|v| v.contains("gap")));
    }

    #[test]
    fn prev_hash_mismatch_detected() {
        let w = wl(3);
        let r1 = make_commitment(&w, 1, None, [1; 32]);
        let r2 = make_commitment(&w, 2, Some([99; 32]), [2; 32]); // wrong prev_hash
        let report = SyncVerifier::verify_received_receipts(&[r1, r2], &w).unwrap();
        assert!(!report.chain_valid);
        assert!(report.violations.iter().any(|v| v.contains("prev_hash")));
    }

    #[test]
    fn empty_receipts_valid() {
        let w = wl(4);
        let report = SyncVerifier::verify_received_receipts(&[], &w).unwrap();
        assert!(report.chain_valid);
        assert_eq!(report.receipts_verified, 0);
    }

    #[test]
    fn wrong_worldline_detected() {
        let w1 = wl(5);
        let w2 = wl(6);
        let r1 = make_commitment(&w2, 1, None, [1; 32]); // wrong worldline
        let report = SyncVerifier::verify_received_receipts(&[r1], &w1).unwrap();
        assert!(!report.chain_valid);
        assert!(report.violations.iter().any(|v| v.contains("worldline")));
    }
}
