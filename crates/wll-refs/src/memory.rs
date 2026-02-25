//! In-memory reference store for testing and ephemeral use.
//!
//! [`InMemoryRefStore`] stores all refs in a `HashMap` protected by a
//! `RwLock`. It implements the full [`RefStore`] trait and is suitable for
//! unit tests, REPL sessions, and short-lived processes.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::error::{RefError, Result};
use crate::names::{validate_branch_name, validate_tag_name};
use crate::traits::RefStore;
use crate::types::{Head, Ref};

/// An in-memory implementation of [`RefStore`].
///
/// All data lives in a `HashMap` behind a `RwLock`. Data is lost when the
/// store is dropped.
#[derive(Debug)]
pub struct InMemoryRefStore {
    refs: RwLock<HashMap<String, Ref>>,
    head: RwLock<Option<Head>>,
}

impl InMemoryRefStore {
    /// Create a new empty ref store.
    pub fn new() -> Self {
        Self {
            refs: RwLock::new(HashMap::new()),
            head: RwLock::new(None),
        }
    }
}

impl Default for InMemoryRefStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RefStore for InMemoryRefStore {
    fn read_ref(&self, name: &str) -> Result<Option<Ref>> {
        let refs = self.refs.read().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        Ok(refs.get(name).cloned())
    }

    fn write_ref(&self, name: &str, reference: &Ref) -> Result<()> {
        // Validate names based on ref type.
        match reference {
            Ref::Branch { name: bname, .. } => {
                validate_branch_name(bname)?;
            }
            Ref::Tag { name: tname, .. } => {
                validate_tag_name(tname)?;
            }
            Ref::Remote { branch, .. } => {
                validate_branch_name(branch)?;
            }
        }

        let mut refs = self.refs.write().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;

        // Tags are immutable: if a tag already exists at this name, reject.
        if reference.is_tag() {
            if let Some(existing) = refs.get(name) {
                if existing.is_tag() {
                    return Err(RefError::TagImmutable {
                        name: name.to_string(),
                    });
                }
            }
        }

        refs.insert(name.to_string(), reference.clone());
        Ok(())
    }

    fn delete_ref(&self, name: &str) -> Result<bool> {
        // Prevent deleting the current branch.
        {
            let head = self.head.read().map_err(|e| {
                RefError::Serialization(format!("lock poisoned: {e}"))
            })?;
            if let Some(Head::Symbolic(current)) = head.as_ref() {
                let head_ref_name = format!("refs/heads/{current}");
                if name == head_ref_name {
                    return Err(RefError::DeleteCurrentBranch {
                        name: current.clone(),
                    });
                }
            }
        }

        let mut refs = self.refs.write().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        Ok(refs.remove(name).is_some())
    }

    fn list_refs(&self, prefix: &str) -> Result<Vec<(String, Ref)>> {
        let refs = self.refs.read().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        let mut result: Vec<(String, Ref)> = refs
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        result.sort_by(|(a, _), (b, _)| a.cmp(b));
        Ok(result)
    }

    fn head(&self) -> Result<Option<Head>> {
        let head = self.head.read().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        Ok(head.clone())
    }

    fn set_head(&self, branch: &str) -> Result<()> {
        validate_branch_name(branch)?;

        let mut head = self.head.write().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        *head = Some(Head::Symbolic(branch.to_string()));
        Ok(())
    }

    fn set_head_detached(&self, receipt_hash: [u8; 32]) -> Result<()> {
        let mut head = self.head.write().map_err(|e| {
            RefError::Serialization(format!("lock poisoned: {e}"))
        })?;
        *head = Some(Head::Detached(receipt_hash));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::{TemporalAnchor, WorldlineId};

    /// Helper to create a test WorldlineId.
    fn test_worldline() -> WorldlineId {
        WorldlineId::from_raw([1u8; 32])
    }

    /// Helper to create a second test WorldlineId.
    fn test_worldline_2() -> WorldlineId {
        WorldlineId::from_raw([2u8; 32])
    }

    /// Helper to create a test branch ref.
    fn test_branch(name: &str, hash: [u8; 32]) -> Ref {
        Ref::Branch {
            name: name.to_string(),
            worldline: test_worldline(),
            receipt_hash: hash,
        }
    }

    /// Helper to create a test tag ref.
    fn test_tag(name: &str, target: [u8; 32]) -> Ref {
        Ref::Tag {
            name: name.to_string(),
            target,
            tagger: test_worldline(),
            message: format!("Release {name}"),
            timestamp: TemporalAnchor::new(1000, 0, 0),
            signature: None,
        }
    }

    /// Helper to create a test remote ref.
    fn test_remote(remote: &str, branch: &str, hash: [u8; 32]) -> Ref {
        Ref::Remote {
            remote: remote.to_string(),
            branch: branch.to_string(),
            worldline: test_worldline_2(),
            receipt_hash: hash,
        }
    }

    // ---- Test 1: Create and read a branch ref ----
    #[test]
    fn create_and_read_branch_ref() {
        let store = InMemoryRefStore::new();
        let branch = test_branch("main", [10u8; 32]);

        store.write_ref("refs/heads/main", &branch).unwrap();

        let read = store.read_ref("refs/heads/main").unwrap();
        assert!(read.is_some());
        let read = read.unwrap();
        assert!(read.is_branch());
        assert_eq!(read.target_hash(), &[10u8; 32]);
    }

    // ---- Test 2: Read non-existent ref returns None ----
    #[test]
    fn read_nonexistent_ref_returns_none() {
        let store = InMemoryRefStore::new();
        let read = store.read_ref("refs/heads/nope").unwrap();
        assert!(read.is_none());
    }

    // ---- Test 3: Delete a branch ref ----
    #[test]
    fn delete_branch_ref() {
        let store = InMemoryRefStore::new();
        let branch = test_branch("feature", [20u8; 32]);

        store.write_ref("refs/heads/feature", &branch).unwrap();
        let deleted = store.delete_ref("refs/heads/feature").unwrap();
        assert!(deleted);

        let read = store.read_ref("refs/heads/feature").unwrap();
        assert!(read.is_none());
    }

    // ---- Test 4: Delete non-existent ref returns false ----
    #[test]
    fn delete_nonexistent_ref_returns_false() {
        let store = InMemoryRefStore::new();
        let deleted = store.delete_ref("refs/heads/ghost").unwrap();
        assert!(!deleted);
    }

    // ---- Test 5: HEAD symbolic ref ----
    #[test]
    fn head_symbolic_ref() {
        let store = InMemoryRefStore::new();

        // Initially HEAD is None.
        assert!(store.head().unwrap().is_none());

        // Set HEAD to main.
        store.set_head("main").unwrap();
        let head = store.head().unwrap().unwrap();
        assert_eq!(head, Head::Symbolic("main".to_string()));
    }

    // ---- Test 6: HEAD detached state ----
    #[test]
    fn head_detached_state() {
        let store = InMemoryRefStore::new();
        let hash = [42u8; 32];

        store.set_head_detached(hash).unwrap();
        let head = store.head().unwrap().unwrap();
        assert_eq!(head, Head::Detached(hash));
    }

    // ---- Test 7: Tag creation ----
    #[test]
    fn create_tag() {
        let store = InMemoryRefStore::new();
        let tag = test_tag("v1.0.0", [30u8; 32]);

        store.write_ref("refs/tags/v1.0.0", &tag).unwrap();

        let read = store.read_ref("refs/tags/v1.0.0").unwrap().unwrap();
        assert!(read.is_tag());
        assert_eq!(read.target_hash(), &[30u8; 32]);
    }

    // ---- Test 8: Tag immutability ----
    #[test]
    fn tag_is_immutable() {
        let store = InMemoryRefStore::new();
        let tag1 = test_tag("v1.0.0", [30u8; 32]);
        let tag2 = test_tag("v1.0.0", [31u8; 32]);

        store.write_ref("refs/tags/v1.0.0", &tag1).unwrap();

        // Second write to the same tag name should fail.
        let err = store.write_ref("refs/tags/v1.0.0", &tag2).unwrap_err();
        assert!(
            matches!(err, RefError::TagImmutable { .. }),
            "expected TagImmutable, got: {err}"
        );
    }

    // ---- Test 9: Remote ref tracking ----
    #[test]
    fn remote_ref_tracking() {
        let store = InMemoryRefStore::new();
        let remote = test_remote("origin", "main", [50u8; 32]);

        store
            .write_ref("refs/remotes/origin/main", &remote)
            .unwrap();

        let read = store
            .read_ref("refs/remotes/origin/main")
            .unwrap()
            .unwrap();
        assert!(read.is_remote());
        assert_eq!(read.target_hash(), &[50u8; 32]);
    }

    // ---- Test 10: List branches ----
    #[test]
    fn list_branches() {
        let store = InMemoryRefStore::new();
        store
            .write_ref("refs/heads/main", &test_branch("main", [1u8; 32]))
            .unwrap();
        store
            .write_ref(
                "refs/heads/develop",
                &test_branch("develop", [2u8; 32]),
            )
            .unwrap();
        store
            .write_ref("refs/tags/v1.0.0", &test_tag("v1.0.0", [3u8; 32]))
            .unwrap();

        let branches = store.branches().unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().any(|(n, _)| n == "refs/heads/main"));
        assert!(branches.iter().any(|(n, _)| n == "refs/heads/develop"));
    }

    // ---- Test 11: List tags ----
    #[test]
    fn list_tags() {
        let store = InMemoryRefStore::new();
        store
            .write_ref("refs/tags/v1.0.0", &test_tag("v1.0.0", [1u8; 32]))
            .unwrap();
        store
            .write_ref("refs/tags/v2.0.0", &test_tag("v2.0.0", [2u8; 32]))
            .unwrap();
        store
            .write_ref("refs/heads/main", &test_branch("main", [3u8; 32]))
            .unwrap();

        let tags = store.tags().unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|(n, _)| n == "refs/tags/v1.0.0"));
        assert!(tags.iter().any(|(n, _)| n == "refs/tags/v2.0.0"));
    }

    // ---- Test 12: List remotes ----
    #[test]
    fn list_remotes() {
        let store = InMemoryRefStore::new();
        store
            .write_ref(
                "refs/remotes/origin/main",
                &test_remote("origin", "main", [1u8; 32]),
            )
            .unwrap();
        store
            .write_ref(
                "refs/remotes/origin/develop",
                &test_remote("origin", "develop", [2u8; 32]),
            )
            .unwrap();
        store
            .write_ref(
                "refs/remotes/upstream/main",
                &test_remote("upstream", "main", [3u8; 32]),
            )
            .unwrap();

        let remotes = store.remotes().unwrap();
        assert_eq!(remotes, vec!["origin", "upstream"]);
    }

    // ---- Test 13: Branch name validation rejects invalid names ----
    #[test]
    fn reject_invalid_branch_name_on_write() {
        let store = InMemoryRefStore::new();
        let bad_branch = Ref::Branch {
            name: "bad..name".to_string(),
            worldline: test_worldline(),
            receipt_hash: [0u8; 32],
        };
        let err = store.write_ref("refs/heads/bad..name", &bad_branch);
        assert!(err.is_err());
    }

    // ---- Test 14: Update branch ref (branches are mutable) ----
    #[test]
    fn update_branch_ref() {
        let store = InMemoryRefStore::new();

        let v1 = test_branch("main", [10u8; 32]);
        store.write_ref("refs/heads/main", &v1).unwrap();

        let v2 = test_branch("main", [20u8; 32]);
        store.write_ref("refs/heads/main", &v2).unwrap();

        let read = store.read_ref("refs/heads/main").unwrap().unwrap();
        assert_eq!(read.target_hash(), &[20u8; 32]);
    }

    // ---- Test 15: Cannot delete current branch ----
    #[test]
    fn cannot_delete_current_branch() {
        let store = InMemoryRefStore::new();
        let branch = test_branch("main", [10u8; 32]);
        store.write_ref("refs/heads/main", &branch).unwrap();
        store.set_head("main").unwrap();

        let err = store.delete_ref("refs/heads/main").unwrap_err();
        assert!(matches!(err, RefError::DeleteCurrentBranch { .. }));
    }

    // ---- Test 16: Nested branch names work ----
    #[test]
    fn nested_branch_names() {
        let store = InMemoryRefStore::new();
        let branch = test_branch("feature/deep/nested", [60u8; 32]);
        store
            .write_ref("refs/heads/feature/deep/nested", &branch)
            .unwrap();

        let read = store
            .read_ref("refs/heads/feature/deep/nested")
            .unwrap()
            .unwrap();
        assert_eq!(read.short_name(), "feature/deep/nested");
    }

    // ---- Test 17: Ref canonical name formatting ----
    #[test]
    fn ref_canonical_names() {
        let branch = test_branch("main", [0u8; 32]);
        assert_eq!(branch.canonical_name(), "refs/heads/main");

        let tag = test_tag("v1.0.0", [0u8; 32]);
        assert_eq!(tag.canonical_name(), "refs/tags/v1.0.0");

        let remote = test_remote("origin", "main", [0u8; 32]);
        assert_eq!(remote.canonical_name(), "refs/remotes/origin/main");
    }

    // ---- Test 18: HEAD switch between branches ----
    #[test]
    fn head_switch_between_branches() {
        let store = InMemoryRefStore::new();
        store.set_head("main").unwrap();
        assert_eq!(
            store.head().unwrap().unwrap(),
            Head::Symbolic("main".to_string())
        );

        store.set_head("develop").unwrap();
        assert_eq!(
            store.head().unwrap().unwrap(),
            Head::Symbolic("develop".to_string())
        );
    }
}
