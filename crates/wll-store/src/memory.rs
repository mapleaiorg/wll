use std::collections::HashMap;
use std::sync::RwLock;

use wll_types::ObjectId;

use crate::error::{StoreError, StoreResult};
use crate::object::StoredObject;
use crate::traits::ObjectStore;

/// In-memory, HashMap-based object store.
///
/// Intended for tests and embedding. All objects are held in memory behind a
/// `RwLock` for safe concurrent access. Objects are cloned on read/write.
pub struct InMemoryObjectStore {
    objects: RwLock<HashMap<ObjectId, StoredObject>>,
}

impl InMemoryObjectStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
        }
    }

    /// Number of objects currently stored.
    pub fn len(&self) -> usize {
        self.objects.read().expect("lock poisoned").len()
    }

    /// Returns `true` if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.objects.read().expect("lock poisoned").is_empty()
    }

    /// Total bytes across all stored objects.
    pub fn total_bytes(&self) -> u64 {
        self.objects
            .read()
            .expect("lock poisoned")
            .values()
            .map(|obj| obj.size)
            .sum()
    }

    /// Remove all objects from the store.
    pub fn clear(&self) {
        self.objects.write().expect("lock poisoned").clear();
    }

    /// Return a sorted list of all object IDs in the store.
    pub fn all_ids(&self) -> Vec<ObjectId> {
        let map = self.objects.read().expect("lock poisoned");
        let mut ids: Vec<ObjectId> = map.keys().copied().collect();
        ids.sort();
        ids
    }
}

impl Default for InMemoryObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStore for InMemoryObjectStore {
    fn read(&self, id: &ObjectId) -> StoreResult<Option<StoredObject>> {
        let map = self.objects.read().expect("lock poisoned");
        Ok(map.get(id).cloned())
    }

    fn write(&self, object: &StoredObject) -> StoreResult<ObjectId> {
        let id = object.compute_id();
        if id.is_null() {
            return Err(StoreError::NullObjectId);
        }
        let mut map = self.objects.write().expect("lock poisoned");
        // Idempotent: if already present, skip (content-addressing guarantees
        // the same ID always maps to the same content).
        map.entry(id).or_insert_with(|| object.clone());
        Ok(id)
    }

    fn exists(&self, id: &ObjectId) -> StoreResult<bool> {
        let map = self.objects.read().expect("lock poisoned");
        Ok(map.contains_key(id))
    }

    fn delete(&self, id: &ObjectId) -> StoreResult<bool> {
        let mut map = self.objects.write().expect("lock poisoned");
        Ok(map.remove(id).is_some())
    }
}

impl std::fmt::Debug for InMemoryObjectStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.len();
        f.debug_struct("InMemoryObjectStore")
            .field("object_count", &count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::*;
    use wll_types::{IdentityMaterial, ReceiptKind, TemporalAnchor, WorldlineId};

    fn make_blob(content: &[u8]) -> StoredObject {
        Blob::new(content.to_vec()).to_stored_object()
    }

    fn make_tree() -> StoredObject {
        let tree = Tree::new(vec![
            TreeEntry::new(
                EntryMode::Regular,
                "hello.txt",
                ObjectId::from_bytes(b"hello"),
            ),
            TreeEntry::new(
                EntryMode::Directory,
                "subdir",
                ObjectId::from_bytes(b"subdir"),
            ),
        ]);
        tree.to_stored_object().unwrap()
    }

    fn make_receipt() -> StoredObject {
        let wid = WorldlineId::derive(&IdentityMaterial::GenesisHash([1u8; 32]));
        let receipt = ReceiptObject {
            worldline: wid,
            seq: 1,
            kind: ReceiptKind::Commitment,
            payload: b"commitment-data".to_vec(),
            receipt_hash: [0xab; 32],
        };
        receipt.to_stored_object().unwrap()
    }

    fn make_snapshot() -> StoredObject {
        let wid = WorldlineId::derive(&IdentityMaterial::GenesisHash([2u8; 32]));
        let snapshot = SnapshotObject {
            worldline: wid,
            anchored_receipt: [0xcc; 32],
            tree_id: ObjectId::from_bytes(b"root"),
            state_hash: [0xdd; 32],
            timestamp: TemporalAnchor::new(1000, 0, 1),
        };
        snapshot.to_stored_object().unwrap()
    }

    // -----------------------------------------------------------------------
    // Core CRUD
    // -----------------------------------------------------------------------

    #[test]
    fn write_and_read_blob() {
        let store = InMemoryObjectStore::new();
        let obj = make_blob(b"hello world");
        let id = store.write(&obj).unwrap();
        assert!(!id.is_null());

        let read_back = store.read(&id).unwrap().expect("should exist");
        assert_eq!(read_back, obj);
    }

    #[test]
    fn write_and_read_tree() {
        let store = InMemoryObjectStore::new();
        let obj = make_tree();
        let id = store.write(&obj).unwrap();

        let read_back = store.read(&id).unwrap().expect("should exist");
        assert_eq!(read_back.kind, ObjectKind::Tree);

        // Decode and verify structure
        let tree = Tree::from_stored_object(&read_back).unwrap();
        assert_eq!(tree.len(), 2);
        assert!(tree.get("hello.txt").is_some());
    }

    #[test]
    fn write_and_read_receipt() {
        let store = InMemoryObjectStore::new();
        let obj = make_receipt();
        let id = store.write(&obj).unwrap();

        let read_back = store.read(&id).unwrap().expect("should exist");
        let decoded = ReceiptObject::from_stored_object(&read_back).unwrap();
        assert_eq!(decoded.seq, 1);
        assert_eq!(decoded.kind, ReceiptKind::Commitment);
    }

    #[test]
    fn write_and_read_snapshot() {
        let store = InMemoryObjectStore::new();
        let obj = make_snapshot();
        let id = store.write(&obj).unwrap();

        let read_back = store.read(&id).unwrap().expect("should exist");
        let decoded = SnapshotObject::from_stored_object(&read_back).unwrap();
        assert_eq!(decoded.timestamp, TemporalAnchor::new(1000, 0, 1));
    }

    // -----------------------------------------------------------------------
    // Content-addressing correctness
    // -----------------------------------------------------------------------

    #[test]
    fn same_content_produces_same_id() {
        let store = InMemoryObjectStore::new();
        let obj1 = make_blob(b"identical content");
        let obj2 = make_blob(b"identical content");
        let id1 = store.write(&obj1).unwrap();
        let id2 = store.write(&obj2).unwrap();
        assert_eq!(id1, id2);
        // Only one object stored (dedup)
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn different_content_produces_different_ids() {
        let store = InMemoryObjectStore::new();
        let id1 = store.write(&make_blob(b"aaa")).unwrap();
        let id2 = store.write(&make_blob(b"bbb")).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(store.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Exists / Delete
    // -----------------------------------------------------------------------

    #[test]
    fn exists_for_missing_object() {
        let store = InMemoryObjectStore::new();
        let id = ObjectId::from_bytes(b"nonexistent");
        assert!(!store.exists(&id).unwrap());
    }

    #[test]
    fn exists_for_present_object() {
        let store = InMemoryObjectStore::new();
        let id = store.write(&make_blob(b"present")).unwrap();
        assert!(store.exists(&id).unwrap());
    }

    #[test]
    fn delete_present_object() {
        let store = InMemoryObjectStore::new();
        let id = store.write(&make_blob(b"to-delete")).unwrap();
        assert!(store.delete(&id).unwrap()); // was present
        assert!(!store.exists(&id).unwrap()); // now gone
        assert!(!store.delete(&id).unwrap()); // second delete = false
    }

    #[test]
    fn delete_missing_object() {
        let store = InMemoryObjectStore::new();
        let id = ObjectId::from_bytes(b"never-written");
        assert!(!store.delete(&id).unwrap());
    }

    #[test]
    fn read_missing_object_returns_none() {
        let store = InMemoryObjectStore::new();
        let id = ObjectId::from_bytes(b"missing");
        assert!(store.read(&id).unwrap().is_none());
    }

    // -----------------------------------------------------------------------
    // Batch operations
    // -----------------------------------------------------------------------

    #[test]
    fn write_batch_and_read_batch() {
        let store = InMemoryObjectStore::new();
        let objects = vec![
            make_blob(b"batch-1"),
            make_blob(b"batch-2"),
            make_blob(b"batch-3"),
        ];
        let ids = store.write_batch(&objects).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(store.len(), 3);

        let read_back = store.read_batch(&ids).unwrap();
        assert_eq!(read_back.len(), 3);
        for (i, maybe_obj) in read_back.into_iter().enumerate() {
            let obj = maybe_obj.expect("batch object should exist");
            assert_eq!(obj, objects[i]);
        }
    }

    #[test]
    fn read_batch_with_missing() {
        let store = InMemoryObjectStore::new();
        let id1 = store.write(&make_blob(b"exists")).unwrap();
        let id2 = ObjectId::from_bytes(b"missing");

        let results = store.read_batch(&[id1, id2]).unwrap();
        assert!(results[0].is_some());
        assert!(results[1].is_none());
    }

    // -----------------------------------------------------------------------
    // Write idempotency
    // -----------------------------------------------------------------------

    #[test]
    fn write_is_idempotent() {
        let store = InMemoryObjectStore::new();
        let obj = make_blob(b"idempotent");
        let id1 = store.write(&obj).unwrap();
        let id2 = store.write(&obj).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(store.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Utility methods
    // -----------------------------------------------------------------------

    #[test]
    fn len_and_is_empty() {
        let store = InMemoryObjectStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.write(&make_blob(b"a")).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn total_bytes() {
        let store = InMemoryObjectStore::new();
        store.write(&make_blob(b"12345")).unwrap(); // 5 bytes
        store.write(&make_blob(b"123456789")).unwrap(); // 9 bytes
        assert_eq!(store.total_bytes(), 14);
    }

    #[test]
    fn clear_removes_all() {
        let store = InMemoryObjectStore::new();
        store.write(&make_blob(b"a")).unwrap();
        store.write(&make_blob(b"b")).unwrap();
        assert_eq!(store.len(), 2);

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn all_ids_is_sorted() {
        let store = InMemoryObjectStore::new();
        let id1 = store.write(&make_blob(b"aaa")).unwrap();
        let id2 = store.write(&make_blob(b"bbb")).unwrap();
        let id3 = store.write(&make_blob(b"ccc")).unwrap();

        let ids = store.all_ids();
        assert_eq!(ids.len(), 3);
        // Verify sorted
        for w in ids.windows(2) {
            assert!(w[0] <= w[1]);
        }
        // All present
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert!(ids.contains(&id3));
    }

    // -----------------------------------------------------------------------
    // Concurrent read safety
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_reads_are_safe() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(InMemoryObjectStore::new());
        let obj = make_blob(b"shared data");
        let id = store.write(&obj).unwrap();

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let expected_id = id;
                thread::spawn(move || {
                    let result = store.read(&expected_id).unwrap();
                    assert!(result.is_some());
                    let read_obj = result.unwrap();
                    assert_eq!(read_obj.compute_id(), expected_id);
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }
    }

    // -----------------------------------------------------------------------
    // Hash verification on read
    // -----------------------------------------------------------------------

    #[test]
    fn stored_object_hash_matches_id() {
        let store = InMemoryObjectStore::new();
        let obj = make_blob(b"verify me");
        let id = store.write(&obj).unwrap();
        let read_back = store.read(&id).unwrap().unwrap();
        // Recompute the hash from the read data and verify it matches
        assert_eq!(read_back.compute_id(), id);
    }

    // -----------------------------------------------------------------------
    // Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn default_creates_empty_store() {
        let store = InMemoryObjectStore::default();
        assert!(store.is_empty());
    }

    // -----------------------------------------------------------------------
    // Debug
    // -----------------------------------------------------------------------

    #[test]
    fn debug_format() {
        let store = InMemoryObjectStore::new();
        store.write(&make_blob(b"x")).unwrap();
        let debug = format!("{store:?}");
        assert!(debug.contains("InMemoryObjectStore"));
        assert!(debug.contains("object_count"));
    }
}
