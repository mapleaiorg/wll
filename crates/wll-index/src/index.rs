//! The core Index structure managing staged entries in memory.
//!
//! The [`Index`] manages a `BTreeMap<String, IndexEntry>` as the staging area.
//! All operations are in-memory; filesystem I/O (walking directories, reading
//! files) is the responsibility of the CLI layer.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;

use wll_store::{
    Blob, EntryMode, ObjectStore, Tree, TreeEntry,
};
use wll_types::ObjectId;

use crate::entry::{IndexEntry, IndexFlags};
use crate::error::{IndexError, IndexResult};
use crate::status::{FileStatus, StatusEntry, WorkdirStatus};

/// The staging index: tracks which files are staged for the next commitment.
///
/// This structure is purely in-memory. It operates on a `BTreeMap` of path
/// to `IndexEntry`. The `store` is used for reading/writing blob and tree
/// objects when building trees or looking up content.
pub struct Index {
    /// The index version.
    pub version: u32,
    /// All tracked entries, keyed by path.
    pub entries: BTreeMap<String, IndexEntry>,
    /// Cached tree ObjectId for the current staged state (invalidated on changes).
    pub tree_cache: Option<ObjectId>,
    /// The object store for reading/writing blobs and trees.
    store: Arc<dyn ObjectStore>,
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("version", &self.version)
            .field("entries", &self.entries.len())
            .field("tree_cache", &self.tree_cache)
            .finish()
    }
}

impl Index {
    /// Create a new empty index backed by the given store.
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self {
            version: 1,
            entries: BTreeMap::new(),
            tree_cache: None,
            store,
        }
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by path.
    pub fn get(&self, path: &str) -> Option<&IndexEntry> {
        self.entries.get(path)
    }

    // ---------------------------------------------------------------
    // Stage operations
    // ---------------------------------------------------------------

    /// Stage a file by adding or updating its entry in the index.
    ///
    /// The caller provides the content as bytes; the index stores the blob
    /// in the object store and creates a staged entry.
    pub fn stage_file(
        &mut self,
        path: &str,
        content: &[u8],
        mode: EntryMode,
    ) -> IndexResult<()> {
        if path.is_empty() {
            return Err(IndexError::InvalidPath("empty path".to_string()));
        }

        // Store the blob.
        let blob = Blob::new(content.to_vec());
        let stored = blob.to_stored_object();
        let object_id = self.store.write(&stored)?;

        let entry = IndexEntry {
            path: path.to_string(),
            object_id,
            mode,
            size: content.len() as u64,
            mtime: SystemTime::now(),
            flags: IndexFlags {
                staged: true,
                modified: false,
                deleted: false,
                conflict: false,
            },
        };

        self.entries.insert(path.to_string(), entry);
        self.tree_cache = None; // invalidate cache
        Ok(())
    }

    /// Stage a file from an already-stored object ID.
    ///
    /// Useful when the blob is already in the store (e.g., during checkout
    /// or merge operations).
    pub fn stage_object(
        &mut self,
        path: &str,
        object_id: ObjectId,
        mode: EntryMode,
        size: u64,
    ) -> IndexResult<()> {
        if path.is_empty() {
            return Err(IndexError::InvalidPath("empty path".to_string()));
        }

        let entry = IndexEntry {
            path: path.to_string(),
            object_id,
            mode,
            size,
            mtime: SystemTime::now(),
            flags: IndexFlags {
                staged: true,
                modified: false,
                deleted: false,
                conflict: false,
            },
        };

        self.entries.insert(path.to_string(), entry);
        self.tree_cache = None;
        Ok(())
    }

    /// Unstage a file (mark it as not staged, but keep it tracked).
    pub fn unstage_file(&mut self, path: &str) -> IndexResult<()> {
        let entry = self
            .entries
            .get_mut(path)
            .ok_or_else(|| IndexError::PathNotFound(path.to_string()))?;

        entry.flags.staged = false;
        self.tree_cache = None;
        Ok(())
    }

    /// Unstage all entries.
    pub fn unstage_all(&mut self) {
        for entry in self.entries.values_mut() {
            entry.flags.staged = false;
        }
        self.tree_cache = None;
    }

    /// Remove an entry from the index entirely.
    pub fn remove(&mut self, path: &str) -> IndexResult<IndexEntry> {
        self.tree_cache = None;
        self.entries
            .remove(path)
            .ok_or_else(|| IndexError::PathNotFound(path.to_string()))
    }

    /// Mark a file as deleted (keeps the entry but flags it).
    pub fn mark_deleted(&mut self, path: &str) -> IndexResult<()> {
        let entry = self
            .entries
            .get_mut(path)
            .ok_or_else(|| IndexError::PathNotFound(path.to_string()))?;

        entry.flags.deleted = true;
        entry.flags.staged = true; // deletion is a staged change
        self.tree_cache = None;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Conflict management
    // ---------------------------------------------------------------

    /// Mark a file as conflicted.
    pub fn mark_conflict(&mut self, path: &str) -> IndexResult<()> {
        let entry = self
            .entries
            .get_mut(path)
            .ok_or_else(|| IndexError::PathNotFound(path.to_string()))?;

        entry.flags.conflict = true;
        entry.flags.staged = false;
        self.tree_cache = None;
        Ok(())
    }

    /// Resolve a conflict on a file by replacing with the given content.
    pub fn resolve_conflict(
        &mut self,
        path: &str,
        object_id: ObjectId,
        size: u64,
    ) -> IndexResult<()> {
        let entry = self
            .entries
            .get_mut(path)
            .ok_or_else(|| IndexError::PathNotFound(path.to_string()))?;

        if !entry.flags.conflict {
            return Err(IndexError::PathNotFound(format!(
                "no conflict at path: {path}"
            )));
        }

        entry.object_id = object_id;
        entry.size = size;
        entry.flags.conflict = false;
        entry.flags.staged = true;
        entry.mtime = SystemTime::now();
        self.tree_cache = None;
        Ok(())
    }

    /// Returns `true` if any entries have unresolved conflicts.
    pub fn has_conflicts(&self) -> bool {
        self.entries.values().any(|e| e.flags.conflict)
    }

    /// Return paths with unresolved conflicts.
    pub fn conflict_paths(&self) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| e.flags.conflict)
            .map(|e| e.path.clone())
            .collect()
    }

    // ---------------------------------------------------------------
    // Status computation
    // ---------------------------------------------------------------

    /// Compute the working directory status from the current index state.
    ///
    /// This only examines the in-memory index entries and their flags.
    /// The caller is responsible for updating flags (modified, deleted)
    /// by comparing with the actual filesystem before calling this.
    pub fn status(&self) -> WorkdirStatus {
        let mut result = WorkdirStatus::new();

        for entry in self.entries.values() {
            if entry.flags.conflict {
                result.conflicts.push(entry.path.clone());
            } else if entry.flags.deleted && entry.flags.staged {
                result.staged.push(StatusEntry::new(
                    &entry.path,
                    FileStatus::Deleted,
                ));
            } else if entry.flags.deleted {
                result.deleted.push(entry.path.clone());
            } else if entry.flags.staged && entry.flags.modified {
                result.staged.push(StatusEntry::new(
                    &entry.path,
                    FileStatus::Modified,
                ));
            } else if entry.flags.staged {
                result.staged.push(StatusEntry::new(
                    &entry.path,
                    FileStatus::New,
                ));
            } else if entry.flags.modified {
                result.modified.push(StatusEntry::new(
                    &entry.path,
                    FileStatus::Modified,
                ));
            }
        }

        result
    }

    // ---------------------------------------------------------------
    // Tree building
    // ---------------------------------------------------------------

    /// Build a `Tree` from all currently staged entries.
    ///
    /// Only entries with `flags.staged == true` and `flags.deleted == false`
    /// are included. Returns the tree's ObjectId after writing it to the store.
    pub fn write_tree(&mut self) -> IndexResult<ObjectId> {
        // Check for conflicts first.
        if self.has_conflicts() {
            let paths = self.conflict_paths();
            return Err(IndexError::UnresolvedConflict(paths.join(", ")));
        }

        // Collect staged, non-deleted entries into tree entries.
        let tree_entries: Vec<TreeEntry> = self
            .entries
            .values()
            .filter(|e| e.flags.staged && !e.flags.deleted)
            .map(|e| TreeEntry::new(e.mode, &e.path, e.object_id))
            .collect();

        let tree = Tree::new(tree_entries);
        let stored = tree.to_stored_object().map_err(|e| {
            IndexError::Serialization(format!("failed to serialize tree: {e}"))
        })?;
        let tree_id = self.store.write(&stored)?;

        self.tree_cache = Some(tree_id);
        Ok(tree_id)
    }

    /// Load entries from an existing tree object in the store.
    ///
    /// Replaces the current index contents with the entries from the tree.
    pub fn read_tree(&mut self, tree_id: &ObjectId) -> IndexResult<()> {
        let stored = self
            .store
            .read(tree_id)?
            .ok_or_else(|| IndexError::ObjectNotFound(*tree_id))?;

        let tree = Tree::from_stored_object(&stored)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;

        self.entries.clear();
        for te in &tree.entries {
            let entry = IndexEntry {
                path: te.name.clone(),
                object_id: te.object_id,
                mode: te.mode,
                size: 0, // size not stored in tree; would need blob lookup
                mtime: SystemTime::now(),
                flags: IndexFlags {
                    staged: false,
                    modified: false,
                    deleted: false,
                    conflict: false,
                },
            };
            self.entries.insert(te.name.clone(), entry);
        }

        self.tree_cache = Some(*tree_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_store::InMemoryObjectStore;

    fn make_store() -> Arc<dyn ObjectStore> {
        Arc::new(InMemoryObjectStore::new())
    }

    fn make_index() -> Index {
        Index::new(make_store())
    }

    #[test]
    fn new_index_is_empty() {
        let idx = make_index();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert!(!idx.has_conflicts());
    }

    #[test]
    fn stage_file_adds_entry() {
        let mut idx = make_index();
        idx.stage_file("hello.txt", b"hello world", EntryMode::Regular)
            .unwrap();

        assert_eq!(idx.len(), 1);
        let entry = idx.get("hello.txt").unwrap();
        assert!(entry.flags.staged);
        assert_eq!(entry.size, 11);
        assert_eq!(entry.mode, EntryMode::Regular);
    }

    #[test]
    fn stage_file_rejects_empty_path() {
        let mut idx = make_index();
        let result = idx.stage_file("", b"data", EntryMode::Regular);
        assert!(matches!(result, Err(IndexError::InvalidPath(_))));
    }

    #[test]
    fn unstage_file_clears_staged_flag() {
        let mut idx = make_index();
        idx.stage_file("file.txt", b"content", EntryMode::Regular)
            .unwrap();
        assert!(idx.get("file.txt").unwrap().flags.staged);

        idx.unstage_file("file.txt").unwrap();
        assert!(!idx.get("file.txt").unwrap().flags.staged);
    }

    #[test]
    fn unstage_nonexistent_file_errors() {
        let mut idx = make_index();
        let result = idx.unstage_file("missing.txt");
        assert!(matches!(result, Err(IndexError::PathNotFound(_))));
    }

    #[test]
    fn unstage_all() {
        let mut idx = make_index();
        idx.stage_file("a.txt", b"aaa", EntryMode::Regular).unwrap();
        idx.stage_file("b.txt", b"bbb", EntryMode::Regular).unwrap();

        assert!(idx.get("a.txt").unwrap().flags.staged);
        assert!(idx.get("b.txt").unwrap().flags.staged);

        idx.unstage_all();

        assert!(!idx.get("a.txt").unwrap().flags.staged);
        assert!(!idx.get("b.txt").unwrap().flags.staged);
    }

    #[test]
    fn remove_entry() {
        let mut idx = make_index();
        idx.stage_file("file.txt", b"content", EntryMode::Regular)
            .unwrap();
        assert_eq!(idx.len(), 1);

        let removed = idx.remove("file.txt").unwrap();
        assert_eq!(removed.path, "file.txt");
        assert!(idx.is_empty());
    }

    #[test]
    fn remove_nonexistent_errors() {
        let mut idx = make_index();
        let result = idx.remove("nope.txt");
        assert!(matches!(result, Err(IndexError::PathNotFound(_))));
    }

    #[test]
    fn mark_deleted() {
        let mut idx = make_index();
        idx.stage_file("file.txt", b"content", EntryMode::Regular)
            .unwrap();
        idx.unstage_file("file.txt").unwrap();

        idx.mark_deleted("file.txt").unwrap();
        let entry = idx.get("file.txt").unwrap();
        assert!(entry.flags.deleted);
        assert!(entry.flags.staged); // deletion is staged
    }

    #[test]
    fn conflict_workflow() {
        let mut idx = make_index();
        idx.stage_file("conflict.txt", b"ours", EntryMode::Regular)
            .unwrap();

        // Mark as conflicted.
        idx.mark_conflict("conflict.txt").unwrap();
        assert!(idx.has_conflicts());
        assert_eq!(idx.conflict_paths(), vec!["conflict.txt".to_string()]);

        // Entry should not be staged while conflicted.
        assert!(!idx.get("conflict.txt").unwrap().flags.staged);

        // Resolve the conflict.
        let new_id = ObjectId::from_bytes(b"resolved content");
        idx.resolve_conflict("conflict.txt", new_id, 16).unwrap();

        assert!(!idx.has_conflicts());
        let entry = idx.get("conflict.txt").unwrap();
        assert!(entry.flags.staged);
        assert!(!entry.flags.conflict);
        assert_eq!(entry.object_id, new_id);
    }

    #[test]
    fn status_staged_entries() {
        let mut idx = make_index();
        idx.stage_file("new.txt", b"new", EntryMode::Regular).unwrap();

        let status = idx.status();
        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].path, "new.txt");
        assert!(matches!(status.staged[0].status, FileStatus::New));
    }

    #[test]
    fn status_deleted_staged() {
        let mut idx = make_index();
        idx.stage_file("gone.txt", b"will be deleted", EntryMode::Regular)
            .unwrap();
        // Unstage first, then mark deleted.
        idx.unstage_file("gone.txt").unwrap();
        idx.mark_deleted("gone.txt").unwrap();

        let status = idx.status();
        assert_eq!(status.staged.len(), 1);
        assert!(matches!(status.staged[0].status, FileStatus::Deleted));
    }

    #[test]
    fn status_conflicted_entries() {
        let mut idx = make_index();
        idx.stage_file("c.txt", b"data", EntryMode::Regular).unwrap();
        idx.mark_conflict("c.txt").unwrap();

        let status = idx.status();
        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0], "c.txt");
    }

    #[test]
    fn status_modified_unstaged() {
        let mut idx = make_index();
        idx.stage_file("mod.txt", b"original", EntryMode::Regular)
            .unwrap();
        // Simulate: unstage and mark as modified externally.
        idx.unstage_file("mod.txt").unwrap();
        idx.entries.get_mut("mod.txt").unwrap().flags.modified = true;

        let status = idx.status();
        assert_eq!(status.modified.len(), 1);
        assert_eq!(status.modified[0].path, "mod.txt");
    }

    #[test]
    fn write_tree_and_read_tree_roundtrip() {
        let store = make_store();
        let mut idx = Index::new(Arc::clone(&store));

        idx.stage_file("alpha.txt", b"alpha content", EntryMode::Regular)
            .unwrap();
        idx.stage_file("beta.txt", b"beta content", EntryMode::Regular)
            .unwrap();

        let tree_id = idx.write_tree().unwrap();
        assert!(!tree_id.is_null());
        assert_eq!(idx.tree_cache, Some(tree_id));

        // Create a fresh index and read the tree back.
        let mut idx2 = Index::new(Arc::clone(&store));
        idx2.read_tree(&tree_id).unwrap();

        assert_eq!(idx2.len(), 2);
        assert!(idx2.get("alpha.txt").is_some());
        assert!(idx2.get("beta.txt").is_some());
    }

    #[test]
    fn write_tree_fails_with_conflicts() {
        let mut idx = make_index();
        idx.stage_file("file.txt", b"data", EntryMode::Regular)
            .unwrap();
        idx.mark_conflict("file.txt").unwrap();

        let result = idx.write_tree();
        assert!(matches!(result, Err(IndexError::UnresolvedConflict(_))));
    }

    #[test]
    fn write_tree_excludes_deleted() {
        let store = make_store();
        let mut idx = Index::new(Arc::clone(&store));

        idx.stage_file("keep.txt", b"keep", EntryMode::Regular)
            .unwrap();
        idx.stage_file("delete.txt", b"delete", EntryMode::Regular)
            .unwrap();
        idx.mark_deleted("delete.txt").unwrap();

        let tree_id = idx.write_tree().unwrap();

        // Read the tree and verify only keep.txt is present.
        let stored_obj = store.read(&tree_id).unwrap().unwrap();
        let tree = Tree::from_stored_object(&stored_obj).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.entries[0].name, "keep.txt");
    }

    #[test]
    fn stage_object_by_id() {
        let store = make_store();
        // Pre-store a blob.
        let blob = Blob::new(b"pre-stored".to_vec());
        let stored_obj = blob.to_stored_object();
        let oid = store.write(&stored_obj).unwrap();

        let mut idx = Index::new(store);
        idx.stage_object("pre.txt", oid, EntryMode::Regular, 10)
            .unwrap();

        let entry = idx.get("pre.txt").unwrap();
        assert_eq!(entry.object_id, oid);
        assert!(entry.flags.staged);
    }

    #[test]
    fn tree_cache_invalidated_on_changes() {
        let store = make_store();
        let mut idx = Index::new(store);

        idx.stage_file("a.txt", b"aaa", EntryMode::Regular).unwrap();
        let _tree_id = idx.write_tree().unwrap();
        assert!(idx.tree_cache.is_some());

        // Stage another file invalidates the cache.
        idx.stage_file("b.txt", b"bbb", EntryMode::Regular).unwrap();
        assert!(idx.tree_cache.is_none());
    }
}
