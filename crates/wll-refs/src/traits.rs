//! The [`RefStore`] trait defining the reference storage interface.
//!
//! Any backend (in-memory, filesystem, database) implements this trait to
//! provide named reference management for the WorldLine Ledger.

use crate::error::Result;
use crate::types::{Head, Ref};

/// Storage backend for named references.
///
/// Implementations must be thread-safe (`Send + Sync`) and provide atomic
/// read/write/delete operations on named refs. The namespace follows a
/// hierarchical layout:
///
/// - `refs/heads/*` for branches
/// - `refs/tags/*` for tags
/// - `refs/remotes/{remote}/*` for remote tracking refs
pub trait RefStore: Send + Sync {
    /// Read a ref by its canonical name (e.g. "refs/heads/main").
    ///
    /// Returns `Ok(None)` if the ref does not exist.
    fn read_ref(&self, name: &str) -> Result<Option<Ref>>;

    /// Write (create or update) a ref at the given canonical name.
    ///
    /// For tags, this should fail if the tag already exists (tags are
    /// immutable). Use `delete_ref` + `write_ref` to replace a tag.
    fn write_ref(&self, name: &str, reference: &Ref) -> Result<()>;

    /// Delete a ref by canonical name.
    ///
    /// Returns `Ok(true)` if the ref existed and was deleted, `Ok(false)` if
    /// it did not exist.
    fn delete_ref(&self, name: &str) -> Result<bool>;

    /// List all refs whose canonical name starts with `prefix`.
    ///
    /// Pass `""` to list all refs. Pass `"refs/heads/"` for branches only.
    fn list_refs(&self, prefix: &str) -> Result<Vec<(String, Ref)>>;

    /// Read the current HEAD state.
    ///
    /// Returns `Ok(None)` if HEAD has not been set.
    fn head(&self) -> Result<Option<Head>>;

    /// Set HEAD to point at a branch (symbolic ref).
    fn set_head(&self, branch: &str) -> Result<()>;

    /// Set HEAD to a detached state pointing directly to a receipt hash.
    fn set_head_detached(&self, receipt_hash: [u8; 32]) -> Result<()>;

    /// List all branch refs.
    fn branches(&self) -> Result<Vec<(String, Ref)>> {
        self.list_refs("refs/heads/")
    }

    /// List all tag refs.
    fn tags(&self) -> Result<Vec<(String, Ref)>> {
        self.list_refs("refs/tags/")
    }

    /// List all known remote names.
    fn remotes(&self) -> Result<Vec<String>> {
        let refs = self.list_refs("refs/remotes/")?;
        let mut remotes: Vec<String> = refs
            .iter()
            .filter_map(|(name, _)| {
                let rest = name.strip_prefix("refs/remotes/")?;
                let remote = rest.split('/').next()?;
                Some(remote.to_string())
            })
            .collect();
        remotes.sort();
        remotes.dedup();
        Ok(remotes)
    }
}
