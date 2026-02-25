//! Diff engine for the WorldLine Ledger.
//!
//! Computes fine-grained diffs between object versions, producing structured
//! change sets for tree comparisons, blob content deltas, and state maps.
//!
//! # Key Types
//!
//! - [`TreeDiff`] / [`TreeChange`] -- Tree-level diff (added/deleted/modified/renamed entries)
//! - [`BlobDiff`] / [`DiffHunk`] / [`DiffLine`] -- Line-level blob diff
//! - [`StateDiff`] / [`StateChange`] -- State map diff (BTreeMap<String, Value>)

pub mod blob_diff;
pub mod error;
pub mod state_diff;
pub mod tree_diff;

pub use blob_diff::{diff_blobs, BlobDiff, DiffHunk, DiffLine};
pub use error::{DiffError, DiffResult};
pub use state_diff::{diff_states, StateDiff, StateChange};
pub use tree_diff::{diff_tree_objects, diff_trees, TreeChange, TreeDiff};
