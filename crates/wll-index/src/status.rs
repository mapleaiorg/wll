//! Working directory status types.
//!
//! These types represent the result of comparing the index state against
//! a known baseline (e.g., the last committed tree).

use serde::{Deserialize, Serialize};

/// Complete status of the working directory relative to the index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkdirStatus {
    /// Files staged for the next commitment.
    pub staged: Vec<StatusEntry>,
    /// Files modified since last staging.
    pub modified: Vec<StatusEntry>,
    /// Files present in the working directory but not tracked.
    pub untracked: Vec<String>,
    /// Files that were tracked but have been deleted.
    pub deleted: Vec<String>,
    /// Files in a conflict state (from merge).
    pub conflicts: Vec<String>,
}

impl WorkdirStatus {
    /// Create an empty status.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if there are no changes of any kind.
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty()
            && self.modified.is_empty()
            && self.untracked.is_empty()
            && self.deleted.is_empty()
            && self.conflicts.is_empty()
    }

    /// Returns `true` if there are any staged changes.
    pub fn has_staged_changes(&self) -> bool {
        !self.staged.is_empty()
    }

    /// Returns `true` if there are any conflicts.
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Total number of entries across all categories.
    pub fn total_entries(&self) -> usize {
        self.staged.len()
            + self.modified.len()
            + self.untracked.len()
            + self.deleted.len()
            + self.conflicts.len()
    }
}

/// A single status entry representing a file change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    /// The file path relative to the workdir root.
    pub path: String,
    /// The kind of change.
    pub status: FileStatus,
}

impl StatusEntry {
    /// Create a new status entry.
    pub fn new(path: impl Into<String>, status: FileStatus) -> Self {
        Self {
            path: path.into(),
            status,
        }
    }
}

/// The kind of file change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    /// A new file that did not previously exist.
    New,
    /// An existing file whose content has changed.
    Modified,
    /// A file that has been removed.
    Deleted,
    /// A file that was renamed from another path.
    Renamed {
        /// The original path before the rename.
        from: String,
    },
    /// A file that was copied from another path.
    Copied {
        /// The source path of the copy.
        from: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_status_is_clean() {
        let status = WorkdirStatus::new();
        assert!(status.is_clean());
        assert!(!status.has_staged_changes());
        assert!(!status.has_conflicts());
        assert_eq!(status.total_entries(), 0);
    }

    #[test]
    fn status_with_staged_is_not_clean() {
        let mut status = WorkdirStatus::new();
        status.staged.push(StatusEntry::new("file.txt", FileStatus::New));
        assert!(!status.is_clean());
        assert!(status.has_staged_changes());
        assert_eq!(status.total_entries(), 1);
    }

    #[test]
    fn status_with_conflicts() {
        let mut status = WorkdirStatus::new();
        status.conflicts.push("conflict.txt".to_string());
        assert!(status.has_conflicts());
        assert!(!status.is_clean());
    }
}
