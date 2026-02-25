use std::collections::HashSet;
use std::path::{Path, PathBuf};

use wll_store::{ObjectStore, StoredObject};
use wll_types::ObjectId;

use crate::error::PackResult;
use crate::reader::PackReader;
use crate::writer::{PackFile, PackWriter};

/// Result of garbage collection.
#[derive(Clone, Debug)]
pub struct GcReport {
    pub objects_removed: usize,
    pub packs_removed: usize,
    pub bytes_freed: u64,
}

/// Manages multiple pack files in a WLL repository.
pub struct PackManager {
    pack_dir: PathBuf,
    packs: Vec<PackReader>,
}

impl PackManager {
    /// Load all packs from a directory.
    pub fn load(wll_dir: &Path) -> PackResult<Self> {
        let pack_dir = wll_dir.join("objects").join("pack");
        let mut packs = Vec::new();

        if pack_dir.exists() {
            for entry in std::fs::read_dir(&pack_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "pack").unwrap_or(false) {
                    match PackReader::open(&path) {
                        Ok(reader) => packs.push(reader),
                        Err(e) => {
                            tracing::warn!("skipping corrupt pack {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(Self { pack_dir, packs })
    }

    /// Create an empty pack manager (for testing).
    pub fn empty() -> Self {
        Self {
            pack_dir: PathBuf::new(),
            packs: Vec::new(),
        }
    }

    /// Read an object from any loaded pack.
    pub fn read_object(&self, id: &ObjectId) -> PackResult<Option<StoredObject>> {
        for pack in &self.packs {
            if let Some(obj) = pack.read_object(id)? {
                return Ok(Some(obj));
            }
        }
        Ok(None)
    }

    /// Check containment across all packs.
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.packs.iter().any(|p| p.contains(id))
    }

    /// Total objects across all packs.
    pub fn total_objects(&self) -> usize {
        self.packs.iter().map(|p| p.object_count()).sum()
    }

    /// Number of loaded packs.
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Repack objects from a store into a single pack.
    pub fn repack(&self, store: &dyn ObjectStore, objects: &[ObjectId]) -> PackResult<PackFile> {
        std::fs::create_dir_all(&self.pack_dir)?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let pack_path = self.pack_dir.join(format!("pack-{ts}"));

        let mut writer = PackWriter::new(&pack_path);
        for id in objects {
            if let Ok(Some(obj)) = store.read(id) {
                writer.add_stored_object(&obj);
            }
        }
        writer.finish()
    }

    /// Garbage collect: report unreachable objects.
    pub fn gc(&self, reachable: &HashSet<ObjectId>) -> GcReport {
        let mut objects_removed = 0;
        for pack in &self.packs {
            for id in pack.object_ids() {
                if !reachable.contains(id) {
                    objects_removed += 1;
                }
            }
        }
        GcReport {
            objects_removed,
            packs_removed: 0,
            bytes_freed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager() {
        let mgr = PackManager::empty();
        assert_eq!(mgr.pack_count(), 0);
        assert_eq!(mgr.total_objects(), 0);
        assert!(!mgr.contains(&ObjectId::null()));
    }

    #[test]
    fn read_from_empty_manager() {
        let mgr = PackManager::empty();
        let result = mgr.read_object(&ObjectId::null()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn gc_empty() {
        let mgr = PackManager::empty();
        let report = mgr.gc(&HashSet::new());
        assert_eq!(report.objects_removed, 0);
    }
}
