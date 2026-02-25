//! Index entry types for tracking working directory files.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use wll_store::EntryMode;
use wll_types::ObjectId;

/// An entry in the staging index, representing a tracked file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Relative path from the workdir root.
    pub path: String,
    /// Content-addressed ID of the file's blob in the object store.
    pub object_id: ObjectId,
    /// File mode (regular, executable, symlink, directory).
    pub mode: EntryMode,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time (used for quick dirty checks).
    pub mtime: SystemTime,
    /// Status flags for this entry.
    pub flags: IndexFlags,
}

impl IndexEntry {
    /// Create a new index entry.
    pub fn new(
        path: impl Into<String>,
        object_id: ObjectId,
        mode: EntryMode,
        size: u64,
    ) -> Self {
        Self {
            path: path.into(),
            object_id,
            mode,
            size,
            mtime: SystemTime::now(),
            flags: IndexFlags::default(),
        }
    }

    /// Create a new staged entry.
    pub fn new_staged(
        path: impl Into<String>,
        object_id: ObjectId,
        mode: EntryMode,
        size: u64,
    ) -> Self {
        let mut entry = Self::new(path, object_id, mode, size);
        entry.flags.staged = true;
        entry
    }
}

/// Status flags for an index entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFlags {
    /// Whether the file is staged for the next commitment.
    pub staged: bool,
    /// Whether the file has been modified relative to the last commitment.
    pub modified: bool,
    /// Whether the file has been deleted from the working directory.
    pub deleted: bool,
    /// Whether the file is in a conflict state (e.g., from a merge).
    pub conflict: bool,
}

impl Default for IndexFlags {
    fn default() -> Self {
        Self {
            staged: false,
            modified: false,
            deleted: false,
            conflict: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entry_has_default_flags() {
        let entry = IndexEntry::new("test.txt", ObjectId::from_bytes(b"test"), EntryMode::Regular, 100);
        assert!(!entry.flags.staged);
        assert!(!entry.flags.modified);
        assert!(!entry.flags.deleted);
        assert!(!entry.flags.conflict);
    }

    #[test]
    fn new_staged_entry_is_staged() {
        let entry = IndexEntry::new_staged(
            "test.txt",
            ObjectId::from_bytes(b"test"),
            EntryMode::Regular,
            100,
        );
        assert!(entry.flags.staged);
        assert!(!entry.flags.modified);
    }
}
