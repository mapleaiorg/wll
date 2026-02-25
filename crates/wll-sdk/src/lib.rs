//! High-level SDK for the WorldLine Ledger.
//!
//! Provides a unified API for programmatic access to all WLL subsystems.
//! This is the main entry point for applications embedding WLL.

pub mod commit;
pub mod error;
pub mod repository;

pub use commit::{CommitProposal, CommitResult, ReceiptSummary};
pub use error::{SdkError, SdkResult};
pub use repository::Wll;

// Re-export key types
pub use wll_types::{ObjectId, WorldlineId, CommitmentClass, CommitmentId};
pub use wll_store::{Tree, TreeEntry, EntryMode, Blob};
pub use wll_ledger::{Receipt, ValidationReport};
