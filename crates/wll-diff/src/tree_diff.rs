//! Tree-level diff: compare two trees and produce a list of changes.
//!
//! Compares entries by name, detecting additions, deletions, modifications,
//! and mode changes. Optionally detects renames based on content similarity.

use std::collections::BTreeMap;

use wll_store::{EntryMode, ObjectStore, Tree, TreeEntry};
use wll_types::ObjectId;

use crate::error::{DiffError, DiffResult};

/// The result of comparing two trees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeDiff {
    /// The list of changes between the old and new trees.
    pub changes: Vec<TreeChange>,
}

impl TreeDiff {
    /// Create an empty tree diff.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }
}

/// A single change between two trees.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeChange {
    /// A new entry was added.
    Added {
        path: String,
        new_id: ObjectId,
        mode: EntryMode,
    },
    /// An existing entry was deleted.
    Deleted {
        path: String,
        old_id: ObjectId,
        mode: EntryMode,
    },
    /// An entry's content changed (same path, different object ID).
    Modified {
        path: String,
        old_id: ObjectId,
        new_id: ObjectId,
        mode: EntryMode,
    },
    /// An entry was renamed (detected by content similarity).
    Renamed {
        old_path: String,
        new_path: String,
        id: ObjectId,
        similarity: f64,
    },
    /// An entry's mode changed but content is the same.
    ModeChanged {
        path: String,
        id: ObjectId,
        old_mode: EntryMode,
        new_mode: EntryMode,
    },
}

/// Compare two trees and produce a diff.
///
/// - `old_tree`: the previous tree (or `None` for an empty tree).
/// - `new_tree`: the current tree ID.
///
/// Both tree objects are read from the store.
pub fn diff_trees(
    store: &dyn ObjectStore,
    old_tree: Option<&ObjectId>,
    new_tree: &ObjectId,
) -> DiffResult<TreeDiff> {
    let old_entries = match old_tree {
        Some(id) => load_tree_entries(store, id)?,
        None => BTreeMap::new(),
    };
    let new_entries = load_tree_entries(store, new_tree)?;

    diff_tree_entries(&old_entries, &new_entries)
}

/// Compare two trees given directly as Tree objects (no store needed).
pub fn diff_tree_objects(
    old_tree: Option<&Tree>,
    new_tree: &Tree,
) -> TreeDiff {
    let old_map = match old_tree {
        Some(t) => entries_to_map(&t.entries),
        None => BTreeMap::new(),
    };
    let new_map = entries_to_map(&new_tree.entries);

    // This never fails since we don't do store I/O.
    diff_tree_entries(&old_map, &new_map).unwrap_or_default()
}

fn entries_to_map(entries: &[TreeEntry]) -> BTreeMap<String, TreeEntry> {
    entries
        .iter()
        .map(|e| (e.name.clone(), e.clone()))
        .collect()
}

fn load_tree_entries(
    store: &dyn ObjectStore,
    tree_id: &ObjectId,
) -> DiffResult<BTreeMap<String, TreeEntry>> {
    let stored = store
        .read(tree_id)?
        .ok_or(DiffError::ObjectNotFound(*tree_id))?;

    let tree = Tree::from_stored_object(&stored)
        .map_err(|e| DiffError::Serialization(e.to_string()))?;

    Ok(entries_to_map(&tree.entries))
}

fn diff_tree_entries(
    old: &BTreeMap<String, TreeEntry>,
    new: &BTreeMap<String, TreeEntry>,
) -> DiffResult<TreeDiff> {
    let mut changes = Vec::new();
    let mut deleted_entries: Vec<(&String, &TreeEntry)> = Vec::new();
    let mut added_entries: Vec<(&String, &TreeEntry)> = Vec::new();

    // Find deleted and modified entries.
    for (name, old_entry) in old {
        match new.get(name) {
            Some(new_entry) => {
                if old_entry.object_id != new_entry.object_id {
                    changes.push(TreeChange::Modified {
                        path: name.clone(),
                        old_id: old_entry.object_id,
                        new_id: new_entry.object_id,
                        mode: new_entry.mode,
                    });
                } else if old_entry.mode != new_entry.mode {
                    changes.push(TreeChange::ModeChanged {
                        path: name.clone(),
                        id: old_entry.object_id,
                        old_mode: old_entry.mode,
                        new_mode: new_entry.mode,
                    });
                }
            }
            None => {
                deleted_entries.push((name, old_entry));
            }
        }
    }

    // Find added entries.
    for (name, new_entry) in new {
        if !old.contains_key(name) {
            added_entries.push((name, new_entry));
        }
    }

    // Rename detection: match deleted + added entries with same object ID.
    let mut matched_deletes = std::collections::HashSet::new();
    let mut matched_adds = std::collections::HashSet::new();

    for (di, (del_name, del_entry)) in deleted_entries.iter().enumerate() {
        for (ai, (add_name, add_entry)) in added_entries.iter().enumerate() {
            if del_entry.object_id == add_entry.object_id
                && !matched_deletes.contains(&di)
                && !matched_adds.contains(&ai)
            {
                changes.push(TreeChange::Renamed {
                    old_path: (*del_name).clone(),
                    new_path: (*add_name).clone(),
                    id: del_entry.object_id,
                    similarity: 1.0,
                });
                matched_deletes.insert(di);
                matched_adds.insert(ai);
            }
        }
    }

    // Remaining unmatched deletes and adds.
    for (di, (name, entry)) in deleted_entries.iter().enumerate() {
        if !matched_deletes.contains(&di) {
            changes.push(TreeChange::Deleted {
                path: (*name).clone(),
                old_id: entry.object_id,
                mode: entry.mode,
            });
        }
    }

    for (ai, (name, entry)) in added_entries.iter().enumerate() {
        if !matched_adds.contains(&ai) {
            changes.push(TreeChange::Added {
                path: (*name).clone(),
                new_id: entry.object_id,
                mode: entry.mode,
            });
        }
    }

    Ok(TreeDiff { changes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_store::InMemoryObjectStore;

    fn oid(b: u8) -> ObjectId {
        ObjectId::from_hash([b; 32])
    }

    fn make_entry(name: &str, id: ObjectId, mode: EntryMode) -> TreeEntry {
        TreeEntry::new(mode, name, id)
    }

    #[test]
    fn empty_to_populated_all_additions() {
        let new_tree = Tree::new(vec![
            make_entry("a.txt", oid(1), EntryMode::Regular),
            make_entry("b.txt", oid(2), EntryMode::Regular),
        ]);

        let diff = diff_tree_objects(None, &new_tree);
        assert_eq!(diff.len(), 2);
        assert!(diff.changes.iter().all(|c| matches!(c, TreeChange::Added { .. })));
    }

    #[test]
    fn populated_to_empty_all_deletions() {
        let old_tree = Tree::new(vec![
            make_entry("a.txt", oid(1), EntryMode::Regular),
            make_entry("b.txt", oid(2), EntryMode::Regular),
        ]);
        let new_tree = Tree::empty();

        let diff = diff_tree_objects(Some(&old_tree), &new_tree);
        assert_eq!(diff.len(), 2);
        assert!(diff.changes.iter().all(|c| matches!(c, TreeChange::Deleted { .. })));
    }

    #[test]
    fn identical_trees_no_changes() {
        let tree = Tree::new(vec![
            make_entry("file.txt", oid(1), EntryMode::Regular),
        ]);

        let diff = diff_tree_objects(Some(&tree), &tree);
        assert!(diff.is_empty());
    }

    #[test]
    fn single_file_modification() {
        let old_tree = Tree::new(vec![
            make_entry("file.txt", oid(1), EntryMode::Regular),
        ]);
        let new_tree = Tree::new(vec![
            make_entry("file.txt", oid(2), EntryMode::Regular),
        ]);

        let diff = diff_tree_objects(Some(&old_tree), &new_tree);
        assert_eq!(diff.len(), 1);
        match &diff.changes[0] {
            TreeChange::Modified { path, old_id, new_id, .. } => {
                assert_eq!(path, "file.txt");
                assert_eq!(*old_id, oid(1));
                assert_eq!(*new_id, oid(2));
            }
            other => panic!("expected Modified, got {:?}", other),
        }
    }

    #[test]
    fn mode_change_detection() {
        let old_tree = Tree::new(vec![
            make_entry("script.sh", oid(1), EntryMode::Regular),
        ]);
        let new_tree = Tree::new(vec![
            make_entry("script.sh", oid(1), EntryMode::Executable),
        ]);

        let diff = diff_tree_objects(Some(&old_tree), &new_tree);
        assert_eq!(diff.len(), 1);
        assert!(matches!(
            &diff.changes[0],
            TreeChange::ModeChanged {
                path,
                old_mode: EntryMode::Regular,
                new_mode: EntryMode::Executable,
                ..
            } if path == "script.sh"
        ));
    }

    #[test]
    fn rename_detection_exact_match() {
        let old_tree = Tree::new(vec![
            make_entry("old_name.txt", oid(1), EntryMode::Regular),
        ]);
        let new_tree = Tree::new(vec![
            make_entry("new_name.txt", oid(1), EntryMode::Regular),
        ]);

        let diff = diff_tree_objects(Some(&old_tree), &new_tree);
        assert_eq!(diff.len(), 1);
        match &diff.changes[0] {
            TreeChange::Renamed { old_path, new_path, similarity, .. } => {
                assert_eq!(old_path, "old_name.txt");
                assert_eq!(new_path, "new_name.txt");
                assert!(*similarity > 0.99);
            }
            other => panic!("expected Renamed, got {:?}", other),
        }
    }

    #[test]
    fn diff_trees_from_store() {
        let store = InMemoryObjectStore::new();

        let old_tree = Tree::new(vec![
            make_entry("file.txt", oid(1), EntryMode::Regular),
        ]);
        let new_tree = Tree::new(vec![
            make_entry("file.txt", oid(2), EntryMode::Regular),
        ]);

        let old_stored = old_tree.to_stored_object().unwrap();
        let new_stored = new_tree.to_stored_object().unwrap();

        let old_id = store.write(&old_stored).unwrap();
        let new_id = store.write(&new_stored).unwrap();

        let diff = diff_trees(&store, Some(&old_id), &new_id).unwrap();
        assert_eq!(diff.len(), 1);
        assert!(matches!(&diff.changes[0], TreeChange::Modified { .. }));
    }

    #[test]
    fn diff_trees_from_none() {
        let store = InMemoryObjectStore::new();

        let new_tree = Tree::new(vec![
            make_entry("file.txt", oid(1), EntryMode::Regular),
        ]);
        let new_stored = new_tree.to_stored_object().unwrap();
        let new_id = store.write(&new_stored).unwrap();

        let diff = diff_trees(&store, None, &new_id).unwrap();
        assert_eq!(diff.len(), 1);
        assert!(matches!(&diff.changes[0], TreeChange::Added { .. }));
    }

    #[test]
    fn mixed_changes() {
        let old_tree = Tree::new(vec![
            make_entry("keep.txt", oid(1), EntryMode::Regular),
            make_entry("modify.txt", oid(2), EntryMode::Regular),
            make_entry("delete.txt", oid(3), EntryMode::Regular),
        ]);
        let new_tree = Tree::new(vec![
            make_entry("keep.txt", oid(1), EntryMode::Regular),
            make_entry("modify.txt", oid(4), EntryMode::Regular),
            make_entry("added.txt", oid(5), EntryMode::Regular),
        ]);

        let diff = diff_tree_objects(Some(&old_tree), &new_tree);
        assert_eq!(diff.len(), 3);

        let has_modified = diff.changes.iter().any(|c| matches!(c, TreeChange::Modified { path, .. } if path == "modify.txt"));
        let has_deleted = diff.changes.iter().any(|c| matches!(c, TreeChange::Deleted { path, .. } if path == "delete.txt"));
        let has_added = diff.changes.iter().any(|c| matches!(c, TreeChange::Added { path, .. } if path == "added.txt"));

        assert!(has_modified, "should detect modification");
        assert!(has_deleted, "should detect deletion");
        assert!(has_added, "should detect addition");
    }
}
