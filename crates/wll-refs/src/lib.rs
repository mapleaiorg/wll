//! Reference management for the WorldLine Ledger.
//!
//! This crate provides named references (branches, tags, HEAD) that point to
//! specific receipts in the WorldLine Ledger. References are the human-readable
//! entry points into the receipt chain, analogous to git refs.
//!
//! # Architecture
//!
//! - **Branches** are mutable pointers to receipt chain tips. They advance as
//!   new receipts are appended.
//! - **Tags** are immutable pointers to specific receipts. Once created, a tag
//!   cannot be moved — delete and recreate if needed.
//! - **Remote refs** track branches on remote nodes and are only updated by
//!   sync operations.
//! - **HEAD** is a symbolic ref that names the current branch, or a detached
//!   ref pointing directly to a receipt hash.
//!
//! # Modules
//!
//! - [`error`] — Error types for ref operations
//! - [`types`] — Core ref types: [`Ref`], [`BranchInfo`], [`Head`]
//! - [`traits`] — The [`RefStore`] trait defining the storage interface
//! - [`names`] — Branch/tag name validation
//! - [`memory`] — In-memory [`InMemoryRefStore`] for tests

pub mod error;
pub mod memory;
pub mod names;
pub mod traits;
pub mod types;

pub use error::{RefError, Result};
pub use memory::InMemoryRefStore;
pub use names::{validate_branch_name, validate_remote_name, validate_tag_name};
pub use traits::RefStore;
pub use types::{BranchInfo, Head, Ref};
