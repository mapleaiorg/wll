use wll_types::ObjectId;

use crate::error::StoreResult;
use crate::object::StoredObject;

/// Content-addressed object store.
///
/// All implementations must satisfy these invariants:
/// - Objects are immutable once written. Content-addressing guarantees this:
///   the same data always produces the same ID.
/// - Write-then-link: write the object, verify the hash, then return the ID.
/// - Concurrent reads are always safe (objects are immutable).
/// - The store never interprets object contents — it is a pure key-value store.
/// - All I/O errors are propagated, never silently ignored.
pub trait ObjectStore: Send + Sync {
    /// Read an object by its content-addressed ID.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    /// Returns `Err` on I/O failure or data corruption.
    fn read(&self, id: &ObjectId) -> StoreResult<Option<StoredObject>>;

    /// Write an object and return its content-addressed ID.
    ///
    /// If the object already exists, this is a no-op (idempotent).
    /// The returned ID is computed from the object's kind and data.
    fn write(&self, object: &StoredObject) -> StoreResult<ObjectId>;

    /// Check whether an object exists in the store.
    fn exists(&self, id: &ObjectId) -> StoreResult<bool>;

    /// Delete an object by ID. Returns `true` if the object existed.
    ///
    /// This is intended for garbage collection only. Deletion of
    /// referenced objects can corrupt the store.
    fn delete(&self, id: &ObjectId) -> StoreResult<bool>;

    /// Read multiple objects in a batch.
    ///
    /// Default implementation calls `read()` for each ID. Backends may
    /// override for better performance (e.g., fewer I/O round-trips).
    fn read_batch(&self, ids: &[ObjectId]) -> StoreResult<Vec<Option<StoredObject>>> {
        ids.iter().map(|id| self.read(id)).collect()
    }

    /// Write multiple objects in a batch and return their IDs.
    ///
    /// Default implementation calls `write()` for each object. Backends may
    /// override for better performance (e.g., single fsync).
    fn write_batch(&self, objects: &[StoredObject]) -> StoreResult<Vec<ObjectId>> {
        objects.iter().map(|obj| self.write(obj)).collect()
    }
}
