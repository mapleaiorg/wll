use serde::{Deserialize, Serialize};
use wll_crypto::ContentHasher;
use wll_types::{ObjectId, ReceiptKind, TemporalAnchor, WorldlineId};

use crate::error::{StoreError, StoreResult};

/// The kind of object stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectKind {
    /// Raw content (file contents, arbitrary data).
    Blob,
    /// Directory listing: ordered entries mapping names to object references.
    Tree,
    /// Serialized receipt stored as an object.
    Receipt,
    /// Serialized worldline state at a point in time.
    Snapshot,
    /// Packed object bundle (for pack storage).
    Pack,
}

impl std::fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blob => write!(f, "blob"),
            Self::Tree => write!(f, "tree"),
            Self::Receipt => write!(f, "receipt"),
            Self::Snapshot => write!(f, "snapshot"),
            Self::Pack => write!(f, "pack"),
        }
    }
}

/// A stored object: kind tag + serialized data + cached size.
///
/// `StoredObject` is the unit of storage. The store never interprets the
/// contents of the data — it is a pure key-value store keyed by content hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredObject {
    /// The type of this object.
    pub kind: ObjectKind,
    /// The serialized bytes of the object.
    pub data: Vec<u8>,
    /// The size of `data` in bytes.
    pub size: u64,
}

impl StoredObject {
    /// Create a new stored object from kind and data.
    pub fn new(kind: ObjectKind, data: Vec<u8>) -> Self {
        let size = data.len() as u64;
        Self { kind, data, size }
    }

    /// Compute the content-addressed ID for this object.
    ///
    /// Uses the appropriate domain-separated hasher for each object kind.
    pub fn compute_id(&self) -> ObjectId {
        let hasher = match self.kind {
            ObjectKind::Blob => &ContentHasher::BLOB,
            ObjectKind::Tree => &ContentHasher::TREE,
            ObjectKind::Receipt => &ContentHasher::RECEIPT,
            ObjectKind::Snapshot | ObjectKind::Pack => &ContentHasher::COMMIT,
        };
        hasher.hash(&self.data)
    }
}

// ---------------------------------------------------------------------------
// Blob
// ---------------------------------------------------------------------------

/// Raw content object (analogous to git blob).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blob {
    pub data: Vec<u8>,
}

impl Blob {
    /// Create a new blob from raw bytes.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Convert into a `StoredObject` for storage.
    pub fn to_stored_object(&self) -> StoredObject {
        StoredObject::new(ObjectKind::Blob, self.data.clone())
    }

    /// Decode from a `StoredObject`.
    pub fn from_stored_object(obj: &StoredObject) -> StoreResult<Self> {
        if obj.kind != ObjectKind::Blob {
            return Err(StoreError::CorruptObject {
                id: obj.compute_id(),
                reason: format!("expected blob, got {}", obj.kind),
            });
        }
        Ok(Self {
            data: obj.data.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// File mode for a tree entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryMode {
    /// Normal file (0o100644).
    Regular,
    /// Executable file (0o100755).
    Executable,
    /// Symbolic link (0o120000).
    Symlink,
    /// Subtree / directory (0o040000).
    Directory,
}

impl EntryMode {
    /// Octal mode value (for display/serialization).
    pub fn mode_bits(&self) -> u32 {
        match self {
            Self::Regular => 0o100644,
            Self::Executable => 0o100755,
            Self::Symlink => 0o120000,
            Self::Directory => 0o040000,
        }
    }

    /// Parse from an octal mode value.
    pub fn from_mode_bits(bits: u32) -> Option<Self> {
        match bits {
            0o100644 => Some(Self::Regular),
            0o100755 => Some(Self::Executable),
            0o120000 => Some(Self::Symlink),
            0o040000 => Some(Self::Directory),
            _ => None,
        }
    }
}

impl std::fmt::Display for EntryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06o}", self.mode_bits())
    }
}

/// A single entry in a tree object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// File mode (regular, executable, symlink, directory).
    pub mode: EntryMode,
    /// Entry name (filename or directory name).
    pub name: String,
    /// Content-addressed ID of the referenced object.
    pub object_id: ObjectId,
}

impl TreeEntry {
    /// Create a new tree entry.
    pub fn new(mode: EntryMode, name: impl Into<String>, object_id: ObjectId) -> Self {
        Self {
            mode,
            name: name.into(),
            object_id,
        }
    }
}

impl PartialOrd for TreeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TreeEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

/// Directory listing object (analogous to git tree).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    /// Sorted entries in this directory.
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Create a new tree with the given entries.
    ///
    /// Entries are sorted by name for deterministic hashing.
    pub fn new(mut entries: Vec<TreeEntry>) -> Self {
        entries.sort();
        Self { entries }
    }

    /// Create an empty tree.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Convert into a `StoredObject` for storage.
    pub fn to_stored_object(&self) -> StoreResult<StoredObject> {
        let data = serde_json::to_vec(self)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(StoredObject::new(ObjectKind::Tree, data))
    }

    /// Decode from a `StoredObject`.
    pub fn from_stored_object(obj: &StoredObject) -> StoreResult<Self> {
        if obj.kind != ObjectKind::Tree {
            return Err(StoreError::CorruptObject {
                id: obj.compute_id(),
                reason: format!("expected tree, got {}", obj.kind),
            });
        }
        serde_json::from_slice(&obj.data)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Look up an entry by name.
    pub fn get(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the tree has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ReceiptObject
// ---------------------------------------------------------------------------

/// Serialized receipt stored as an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptObject {
    /// The worldline this receipt belongs to.
    pub worldline: WorldlineId,
    /// Sequence number within the stream.
    pub seq: u64,
    /// Kind of receipt.
    pub kind: ReceiptKind,
    /// Bincode-serialized receipt payload.
    pub payload: Vec<u8>,
    /// BLAKE3 hash of the receipt content (for chain integrity).
    pub receipt_hash: [u8; 32],
}

impl ReceiptObject {
    /// Convert into a `StoredObject` for storage.
    pub fn to_stored_object(&self) -> StoreResult<StoredObject> {
        let data = serde_json::to_vec(self)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(StoredObject::new(ObjectKind::Receipt, data))
    }

    /// Decode from a `StoredObject`.
    pub fn from_stored_object(obj: &StoredObject) -> StoreResult<Self> {
        if obj.kind != ObjectKind::Receipt {
            return Err(StoreError::CorruptObject {
                id: obj.compute_id(),
                reason: format!("expected receipt, got {}", obj.kind),
            });
        }
        serde_json::from_slice(&obj.data)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// SnapshotObject
// ---------------------------------------------------------------------------

/// Serialized worldline state at a point in time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotObject {
    /// The worldline this snapshot belongs to.
    pub worldline: WorldlineId,
    /// Hash of the latest anchored receipt.
    pub anchored_receipt: [u8; 32],
    /// Root tree of the snapshot.
    pub tree_id: ObjectId,
    /// Hash of the complete worldline state.
    pub state_hash: [u8; 32],
    /// Timestamp when the snapshot was taken.
    pub timestamp: TemporalAnchor,
}

impl SnapshotObject {
    /// Convert into a `StoredObject` for storage.
    pub fn to_stored_object(&self) -> StoreResult<StoredObject> {
        let data = serde_json::to_vec(self)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(StoredObject::new(ObjectKind::Snapshot, data))
    }

    /// Decode from a `StoredObject`.
    pub fn from_stored_object(obj: &StoredObject) -> StoreResult<Self> {
        if obj.kind != ObjectKind::Snapshot {
            return Err(StoreError::CorruptObject {
                id: obj.compute_id(),
                reason: format!("expected snapshot, got {}", obj.kind),
            });
        }
        serde_json::from_slice(&obj.data)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::IdentityMaterial;

    #[test]
    fn blob_roundtrip() {
        let blob = Blob::new(b"hello world".to_vec());
        let stored = blob.to_stored_object();
        let decoded = Blob::from_stored_object(&stored).unwrap();
        assert_eq!(blob, decoded);
    }

    #[test]
    fn blob_kind_mismatch() {
        let stored = StoredObject::new(ObjectKind::Tree, b"not a tree".to_vec());
        let err = Blob::from_stored_object(&stored).unwrap_err();
        assert!(matches!(err, StoreError::CorruptObject { .. }));
    }

    #[test]
    fn tree_entries_sorted() {
        let entries = vec![
            TreeEntry::new(EntryMode::Regular, "zebra.txt", ObjectId::null()),
            TreeEntry::new(EntryMode::Regular, "alpha.txt", ObjectId::null()),
            TreeEntry::new(EntryMode::Directory, "middle", ObjectId::null()),
        ];
        let tree = Tree::new(entries);
        assert_eq!(tree.entries[0].name, "alpha.txt");
        assert_eq!(tree.entries[1].name, "middle");
        assert_eq!(tree.entries[2].name, "zebra.txt");
    }

    #[test]
    fn tree_roundtrip() {
        let entries = vec![
            TreeEntry::new(EntryMode::Regular, "file.txt", ObjectId::from_bytes(b"content")),
            TreeEntry::new(EntryMode::Directory, "subdir", ObjectId::from_bytes(b"tree")),
        ];
        let tree = Tree::new(entries);
        let stored = tree.to_stored_object().unwrap();
        let decoded = Tree::from_stored_object(&stored).unwrap();
        assert_eq!(tree, decoded);
    }

    #[test]
    fn tree_get_entry() {
        let tree = Tree::new(vec![
            TreeEntry::new(EntryMode::Regular, "a.txt", ObjectId::null()),
            TreeEntry::new(EntryMode::Regular, "b.txt", ObjectId::from_bytes(b"b")),
        ]);
        assert!(tree.get("a.txt").is_some());
        assert!(tree.get("missing").is_none());
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn empty_tree() {
        let tree = Tree::empty();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn entry_mode_bits_roundtrip() {
        for mode in [
            EntryMode::Regular,
            EntryMode::Executable,
            EntryMode::Symlink,
            EntryMode::Directory,
        ] {
            let bits = mode.mode_bits();
            let parsed = EntryMode::from_mode_bits(bits).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn entry_mode_unknown_bits() {
        assert!(EntryMode::from_mode_bits(0o777).is_none());
    }

    #[test]
    fn receipt_object_roundtrip() {
        let wid = WorldlineId::derive(&IdentityMaterial::GenesisHash([42u8; 32]));
        let receipt = ReceiptObject {
            worldline: wid,
            seq: 1,
            kind: ReceiptKind::Commitment,
            payload: b"test payload".to_vec(),
            receipt_hash: [0xab; 32],
        };
        let stored = receipt.to_stored_object().unwrap();
        let decoded = ReceiptObject::from_stored_object(&stored).unwrap();
        assert_eq!(receipt, decoded);
    }

    #[test]
    fn snapshot_object_roundtrip() {
        let wid = WorldlineId::derive(&IdentityMaterial::GenesisHash([7u8; 32]));
        let snapshot = SnapshotObject {
            worldline: wid,
            anchored_receipt: [0xcc; 32],
            tree_id: ObjectId::from_bytes(b"root tree"),
            state_hash: [0xdd; 32],
            timestamp: TemporalAnchor::new(1000, 0, 1),
        };
        let stored = snapshot.to_stored_object().unwrap();
        let decoded = SnapshotObject::from_stored_object(&stored).unwrap();
        assert_eq!(snapshot, decoded);
    }

    #[test]
    fn stored_object_id_deterministic() {
        let obj = StoredObject::new(ObjectKind::Blob, b"deterministic".to_vec());
        let id1 = obj.compute_id();
        let id2 = obj.compute_id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_kinds_produce_different_ids() {
        let data = b"same data".to_vec();
        let blob = StoredObject::new(ObjectKind::Blob, data.clone());
        let tree = StoredObject::new(ObjectKind::Tree, data.clone());
        let receipt = StoredObject::new(ObjectKind::Receipt, data);
        assert_ne!(blob.compute_id(), tree.compute_id());
        assert_ne!(blob.compute_id(), receipt.compute_id());
    }

    #[test]
    fn object_kind_display() {
        assert_eq!(format!("{}", ObjectKind::Blob), "blob");
        assert_eq!(format!("{}", ObjectKind::Tree), "tree");
        assert_eq!(format!("{}", ObjectKind::Receipt), "receipt");
        assert_eq!(format!("{}", ObjectKind::Snapshot), "snapshot");
        assert_eq!(format!("{}", ObjectKind::Pack), "pack");
    }
}
