//! Staging index for the WorldLine Ledger.
//!
//! Tracks the working tree state, detects file changes via content hashing,
//! and maintains the staging area between the working directory and the next
//! commitment.
//!
//! # Key Types
//!
//! - [`Index`] -- The in-memory staging area (BTreeMap-backed)
//! - [`IndexEntry`] -- A tracked file entry with flags
//! - [`IndexFlags`] -- Staged/modified/deleted/conflict flags
//! - [`WorkdirStatus`] -- Result of status computation
//! - [`FileStatus`] -- Kind of change (New, Modified, Deleted, etc.)

pub mod entry;
pub mod error;
pub mod index;
pub mod status;

pub use entry::{IndexEntry, IndexFlags};
pub use error::{IndexError, IndexResult};
pub use index::Index;
pub use status::{FileStatus, StatusEntry, WorkdirStatus};
