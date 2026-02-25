use std::collections::BTreeMap;

use serde_json::Value;
use wll_types::{
    CommitmentId, IdentityMaterial, ObjectId, WorldlineId,
};
use wll_store::{Blob, InMemoryObjectStore, ObjectStore, StoredObject, Tree, TreeEntry};
use wll_ledger::{
    CommitmentProposal, Decision, EvidenceBundle, InMemoryLedger, LedgerReader, LedgerWriter,
    OutcomeRecord, Receipt, ReplayEngine, ReplayResult, LatestStateProjection,
    ProjectionBuilder, StateUpdate, StreamValidator, ValidationReport,
};
use wll_refs::{Head, InMemoryRefStore, Ref, RefStore};
use wll_dag::ProvenanceDag;

use crate::commit::{CommitProposal as SdkProposal, CommitResult, ReceiptSummary};
use crate::error::{SdkError, SdkResult};

/// High-level WLL repository API.
pub struct Wll {
    worldline: WorldlineId,
    store: InMemoryObjectStore,
    ledger: InMemoryLedger,
    refs: InMemoryRefStore,
    _dag: ProvenanceDag,
}

impl Wll {
    /// Initialize a new WLL repository with a random worldline.
    pub fn init() -> SdkResult<Self> {
        let seed = time_based_seed();
        let worldline = WorldlineId::derive(&IdentityMaterial::GenesisHash(seed));
        Self::init_inner(worldline)
    }

    /// Initialize with a specific worldline ID.
    pub fn init_with_worldline(worldline: WorldlineId) -> SdkResult<Self> {
        Self::init_inner(worldline)
    }

    fn init_inner(worldline: WorldlineId) -> SdkResult<Self> {
        let store = InMemoryObjectStore::new();
        let ledger = InMemoryLedger::default();
        let refs = InMemoryRefStore::new();

        // Create the main branch ref
        let branch_ref = Ref::Branch {
            name: "main".into(),
            worldline: worldline.clone(),
            receipt_hash: [0; 32],
        };
        refs.write_ref("refs/heads/main", &branch_ref)
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        refs.set_head("main")
            .map_err(|e| SdkError::Internal(e.to_string()))?;

        Ok(Self {
            worldline,
            store,
            ledger,
            refs,
            _dag: ProvenanceDag::new(),
        })
    }

    // ---- Content operations ----

    pub fn write_blob(&self, data: &[u8]) -> SdkResult<ObjectId> {
        let blob = Blob::new(data.to_vec());
        let stored = blob.to_stored_object();
        let id = self.store.write(&stored)?;
        Ok(id)
    }

    pub fn read_blob(&self, id: &ObjectId) -> SdkResult<Vec<u8>> {
        let obj = self.store.read(id)?
            .ok_or_else(|| SdkError::ObjectNotFound(id.to_hex()))?;
        let blob = Blob::from_stored_object(&obj)?;
        Ok(blob.data)
    }

    pub fn write_tree(&self, entries: Vec<TreeEntry>) -> SdkResult<ObjectId> {
        let tree = Tree::new(entries);
        let stored = tree.to_stored_object()?;
        let id = self.store.write(&stored)?;
        Ok(id)
    }

    pub fn read_tree(&self, id: &ObjectId) -> SdkResult<Tree> {
        let obj = self.store.read(id)?
            .ok_or_else(|| SdkError::ObjectNotFound(id.to_hex()))?;
        let tree = Tree::from_stored_object(&obj)?;
        Ok(tree)
    }

    // ---- Commitment operations ----

    pub fn commit(&self, proposal: SdkProposal) -> SdkResult<CommitResult> {
        let evidence = if proposal.evidence.is_empty() {
            EvidenceBundle::empty()
        } else {
            EvidenceBundle::from_references(proposal.evidence.clone())
        };

        let ledger_proposal = CommitmentProposal {
            worldline: self.worldline.clone(),
            commitment_id: CommitmentId::new(),
            class: proposal.effective_class(),
            intent: proposal.effective_intent().to_string(),
            requested_caps: vec![],
            targets: vec![self.worldline.clone()],
            evidence,
            nonce: time_nonce(),
        };

        let commitment = self.ledger.append_commitment(
            &ledger_proposal,
            &Decision::Accepted,
            [0; 32],
        )?;

        let outcome_record = OutcomeRecord {
            effects: vec![],
            proofs: vec![],
            state_updates: vec![StateUpdate {
                key: "message".into(),
                value: Value::String(proposal.message.clone()),
            }],
            metadata: BTreeMap::new(),
        };

        let outcome = self.ledger.append_outcome(
            commitment.receipt_hash,
            &outcome_record,
        )?;

        // Update branch tip
        let branch = self.current_branch()?;
        let branch_ref = Ref::Branch {
            name: branch.clone(),
            worldline: self.worldline.clone(),
            receipt_hash: outcome.receipt_hash,
        };
        self.refs.write_ref(&format!("refs/heads/{branch}"), &branch_ref)
            .map_err(|e| SdkError::Internal(e.to_string()))?;

        Ok(CommitResult {
            receipt_hash: outcome.receipt_hash,
            commitment_receipt: commitment,
            outcome_receipt: outcome,
        })
    }

    pub fn log(&self, limit: usize) -> SdkResult<Vec<ReceiptSummary>> {
        let receipts = self.ledger.read_all(&self.worldline)?;
        let summaries = receipts
            .iter()
            .rev()
            .take(limit)
            .map(|r| {
                let (intent, accepted) = match r {
                    Receipt::Commitment(c) => (Some(c.intent.clone()), Some(c.decision.is_accepted())),
                    Receipt::Outcome(o) => (None, Some(o.accepted)),
                    Receipt::Snapshot(_) => (None, None),
                };
                ReceiptSummary {
                    seq: r.seq(),
                    receipt_hash: r.receipt_hash(),
                    kind: format!("{:?}", r.kind()),
                    intent,
                    accepted,
                    timestamp_ms: r.timestamp().physical_ms,
                }
            })
            .collect();
        Ok(summaries)
    }

    pub fn show(&self, receipt_hash: &[u8; 32]) -> SdkResult<Receipt> {
        let receipt = self.ledger.get_by_hash(*receipt_hash)?
            .ok_or_else(|| SdkError::ObjectNotFound(hex::encode(receipt_hash)))?;
        Ok(receipt)
    }

    // ---- Branch operations ----

    pub fn create_branch(&self, name: &str) -> SdkResult<()> {
        let head = self.ledger.head(&self.worldline)?;
        let tip = head.map(|r| r.receipt_hash).unwrap_or([0; 32]);
        let branch_ref = Ref::Branch {
            name: name.into(),
            worldline: self.worldline.clone(),
            receipt_hash: tip,
        };
        self.refs.write_ref(&format!("refs/heads/{name}"), &branch_ref)
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        Ok(())
    }

    pub fn switch_branch(&self, name: &str) -> SdkResult<()> {
        let existing = self.refs.read_ref(&format!("refs/heads/{name}"))
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        if existing.is_none() {
            return Err(SdkError::BranchNotFound(name.into()));
        }
        self.refs.set_head(name)
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        Ok(())
    }

    pub fn current_branch(&self) -> SdkResult<String> {
        let head = self.refs.head()
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        match head {
            Some(Head::Symbolic(name)) => Ok(name),
            Some(Head::Detached(_)) => Err(SdkError::InvalidOperation("HEAD is detached".into())),
            None => Err(SdkError::InvalidOperation("HEAD not set".into())),
        }
    }

    pub fn list_branches(&self) -> SdkResult<Vec<String>> {
        let branches = self.refs.branches()
            .map_err(|e| SdkError::Internal(e.to_string()))?;
        Ok(branches.into_iter().map(|(name, _)| name).collect())
    }

    // ---- Provenance queries ----

    pub fn verify(&self) -> SdkResult<ValidationReport> {
        let report = StreamValidator::validate_stream(&self.ledger, &self.worldline)?;
        Ok(report)
    }

    pub fn replay(&self) -> SdkResult<ReplayResult> {
        let result = ReplayEngine::replay_from_genesis(&self.ledger, &self.worldline)?;
        Ok(result)
    }

    pub fn latest_state(&self) -> SdkResult<LatestStateProjection> {
        let projection = ProjectionBuilder::latest_state(&self.ledger, &self.worldline)?;
        Ok(projection)
    }

    // ---- Accessors ----

    pub fn worldline(&self) -> &WorldlineId { &self.worldline }
    pub fn store(&self) -> &InMemoryObjectStore { &self.store }
    pub fn ledger(&self) -> &InMemoryLedger { &self.ledger }

    pub fn receipt_count(&self) -> SdkResult<u64> {
        let count = self.ledger.receipt_count(&self.worldline)?;
        Ok(count)
    }
}

fn time_based_seed() -> [u8; 32] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut seed = [0u8; 32];
    let bytes = t.to_le_bytes();
    seed[..bytes.len().min(32)].copy_from_slice(&bytes[..bytes.len().min(32)]);
    *blake3::hash(&seed).as_bytes()
}

fn time_nonce() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_store::EntryMode;

    fn wl_seed(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    #[test]
    fn init_creates_repo() {
        let wll = Wll::init().unwrap();
        assert_eq!(wll.current_branch().unwrap(), "main");
        assert_eq!(wll.receipt_count().unwrap(), 0);
    }

    #[test]
    fn init_with_worldline() {
        let wid = wl_seed(42);
        let wll = Wll::init_with_worldline(wid.clone()).unwrap();
        assert_eq!(wll.worldline(), &wid);
    }

    #[test]
    fn blob_roundtrip() {
        let wll = Wll::init().unwrap();
        let id = wll.write_blob(b"hello").unwrap();
        let data = wll.read_blob(&id).unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn tree_roundtrip() {
        let wll = Wll::init().unwrap();
        let blob_id = wll.write_blob(b"content").unwrap();
        let entries = vec![TreeEntry::new(EntryMode::Regular, "file.txt", blob_id)];
        let tree_id = wll.write_tree(entries).unwrap();
        let tree = wll.read_tree(&tree_id).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.entries[0].name, "file.txt");
    }

    #[test]
    fn commit_creates_receipt_pair() {
        let wll = Wll::init().unwrap();
        let result = wll.commit(SdkProposal::new("Initial commit")).unwrap();
        assert_eq!(result.commitment_receipt.seq, 1);
        assert_eq!(result.outcome_receipt.seq, 2);
        assert_eq!(wll.receipt_count().unwrap(), 2);
    }

    #[test]
    fn commit_updates_branch_tip() {
        let wll = Wll::init().unwrap();
        let result = wll.commit(SdkProposal::new("commit 1")).unwrap();
        let branch_ref = wll.refs.read_ref("refs/heads/main").unwrap().unwrap();
        assert_eq!(*branch_ref.target_hash(), result.receipt_hash);
    }

    #[test]
    fn multiple_commits() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("first")).unwrap();
        wll.commit(SdkProposal::new("second")).unwrap();
        assert_eq!(wll.receipt_count().unwrap(), 4); // 2 commits * 2 receipts each
    }

    #[test]
    fn log_returns_reverse() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("first")).unwrap();
        wll.commit(SdkProposal::new("second")).unwrap();
        let log = wll.log(10).unwrap();
        assert_eq!(log.len(), 4);
        assert!(log[0].seq > log[1].seq);
    }

    #[test]
    fn log_respects_limit() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("a")).unwrap();
        wll.commit(SdkProposal::new("b")).unwrap();
        let log = wll.log(2).unwrap();
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn show_receipt() {
        let wll = Wll::init().unwrap();
        let result = wll.commit(SdkProposal::new("test")).unwrap();
        let receipt = wll.show(&result.commitment_receipt.receipt_hash).unwrap();
        assert!(receipt.as_commitment().is_some());
    }

    #[test]
    fn show_missing_receipt() {
        let wll = Wll::init().unwrap();
        let err = wll.show(&[0xFF; 32]).unwrap_err();
        assert!(matches!(err, SdkError::ObjectNotFound(_)));
    }

    #[test]
    fn create_and_list_branches() {
        let wll = Wll::init().unwrap();
        wll.create_branch("feature").unwrap();
        let branches = wll.list_branches().unwrap();
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn switch_branch() {
        let wll = Wll::init().unwrap();
        wll.create_branch("dev").unwrap();
        wll.switch_branch("dev").unwrap();
        assert_eq!(wll.current_branch().unwrap(), "dev");
    }

    #[test]
    fn switch_missing_branch() {
        let wll = Wll::init().unwrap();
        let err = wll.switch_branch("nonexistent").unwrap_err();
        assert!(matches!(err, SdkError::BranchNotFound(_)));
    }

    #[test]
    fn verify_empty_chain() {
        let wll = Wll::init().unwrap();
        let report = wll.verify().unwrap();
        assert!(report.is_valid());
    }

    #[test]
    fn verify_after_commits() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("a")).unwrap();
        wll.commit(SdkProposal::new("b")).unwrap();
        let report = wll.verify().unwrap();
        assert!(report.is_valid());
    }

    #[test]
    fn replay_from_genesis() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("init")).unwrap();
        let result = wll.replay().unwrap();
        assert_eq!(result.applied_outcomes, 1);
    }

    #[test]
    fn latest_state() {
        let wll = Wll::init().unwrap();
        wll.commit(SdkProposal::new("state test")).unwrap();
        let state = wll.latest_state().unwrap();
        assert!(state.trajectory_length > 0);
    }
}
