//! Content-addressed object storage for the WorldLine Ledger.
//!
//! This crate implements a hash-keyed object store analogous to git's
//! `.git/objects/` directory. Every piece of data in WLL -- blobs, trees,
//! receipts, snapshots -- is stored as an immutable object identified by its
//! BLAKE3 hash (domain-separated by object kind).
//!
//! # Object Types
//!
//! - [`Blob`] -- raw content (file contents, arbitrary data)
//! - [`Tree`] -- directory listing mapping names to object references
//! - [`ReceiptObject`] -- serialized receipt for chain integrity
//! - [`SnapshotObject`] -- point-in-time worldline state
//!
//! # Storage Backends
//!
//! All backends implement the [`ObjectStore`] trait:
//!
//! - [`InMemoryObjectStore`] -- `HashMap`-based store for tests and embedding
//!
//! # Design Rules
//!
//! 1. Objects are immutable once written (content-addressing guarantees this).
//! 2. Write-then-link: write object, verify hash, then update references.
//! 3. Concurrent reads are always safe (objects are immutable).
//! 4. Writes are serialized per-stream but parallel across streams.
//! 5. The store never interprets object contents -- it is a pure key-value store.
//! 6. All I/O errors are propagated, never silently ignored.

pub mod error;
pub mod memory;
pub mod object;
pub mod traits;

// Re-export primary types at crate root for ergonomic imports.
pub use error::{StoreError, StoreResult};
pub use memory::InMemoryObjectStore;
pub use object::{
    Blob, EntryMode, ObjectKind, ReceiptObject, SnapshotObject, StoredObject, Tree, TreeEntry,
};
pub use traits::ObjectStore;
