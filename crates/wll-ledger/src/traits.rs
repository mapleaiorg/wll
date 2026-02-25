use wll_types::WorldlineId;

use crate::error::LedgerError;
use crate::records::{
    CommitmentProposal, CommitmentReceipt, Decision, OutcomeReceipt, OutcomeRecord, Receipt,
    ReceiptRef, SnapshotInput, SnapshotReceipt,
};

/// Write boundary for WorldLine Ledger append operations.
pub trait LedgerWriter: Send + Sync {
    fn append_commitment(
        &self,
        proposal: &CommitmentProposal,
        decision: &Decision,
        policy_hash: [u8; 32],
    ) -> Result<CommitmentReceipt, LedgerError>;

    fn append_outcome(
        &self,
        commitment_receipt_hash: [u8; 32],
        outcome: &OutcomeRecord,
    ) -> Result<OutcomeReceipt, LedgerError>;

    fn append_rejection_outcome(
        &self,
        commitment_receipt_hash: [u8; 32],
        reason: &str,
    ) -> Result<OutcomeReceipt, LedgerError>;

    fn append_snapshot(&self, snapshot: &SnapshotInput) -> Result<SnapshotReceipt, LedgerError>;
}

/// Read boundary for WorldLine Ledger query/replay operations.
pub trait LedgerReader: Send + Sync {
    fn head(&self, worldline: &WorldlineId) -> Result<Option<ReceiptRef>, LedgerError>;

    fn read_range(
        &self,
        worldline: &WorldlineId,
        from_seq: u64,
        to_seq: u64,
    ) -> Result<Vec<Receipt>, LedgerError>;

    fn read_all(&self, worldline: &WorldlineId) -> Result<Vec<Receipt>, LedgerError>;

    fn get_by_hash(&self, hash: [u8; 32]) -> Result<Option<Receipt>, LedgerError>;

    fn worldlines(&self) -> Result<Vec<WorldlineId>, LedgerError>;

    fn receipt_count(&self, worldline: &WorldlineId) -> Result<u64, LedgerError>;
}
