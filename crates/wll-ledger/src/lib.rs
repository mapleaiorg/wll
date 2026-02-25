//! Append-only receipt ledger for the WorldLine Ledger (WLL).
//!
//! This crate is the heart of WLL. It provides:
//! - Commitment and outcome receipt types with hash-linked integrity
//! - `LedgerWriter` / `LedgerReader` trait boundaries
//! - `InMemoryLedger` implementation for tests and embedding
//! - Deterministic replay from genesis or snapshot
//! - Projection builders (latest state, audit index)
//! - Stream validation (hash chain, sequence, attribution)

pub mod error;
pub mod memory;
pub mod projection;
pub mod records;
pub mod replay;
pub mod traits;
pub mod validation;

pub use error::LedgerError;
pub use memory::InMemoryLedger;
pub use projection::{
    AuditIndexEntry, AuditIndexProjection, LatestStateProjection, ProjectionBuilder,
};
pub use records::{
    CommitmentProposal, CommitmentReceipt, Decision, EffectSummary, EvidenceBundle, OutcomeReceipt,
    OutcomeRecord, ProofRef, Receipt, ReceiptKind, ReceiptRef, SnapshotInput, SnapshotReceipt,
    StateUpdate,
};
pub use replay::{ReplayEngine, ReplayResult};
pub use traits::{LedgerReader, LedgerWriter};
pub use validation::{StreamValidator, ValidationReport, Violation};
