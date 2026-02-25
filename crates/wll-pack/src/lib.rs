//! Pack file format for the WorldLine Ledger.
//!
//! Provides zstd-compressed, CRC-checked pack files for efficient storage and
//! network transfer of objects, with delta compression and memory-mapped indexing.
//!
//! # Architecture
//!
//! - **Pack file** (`.pack`): concatenated compressed objects with a BLAKE3 checksum
//! - **Pack index** (`.idx`): fan-out table + sorted IDs for O(log n) lookups
//! - **PackWriter**: builds packs from loose objects
//! - **PackReader**: random-access reading using the index
//! - **PackManager**: manages multiple packs, repack, and GC

pub mod entry;
pub mod error;
pub mod index;
pub mod manager;
pub mod reader;
pub mod writer;

pub use entry::{PackEntry, PackObjectKind};
pub use error::{PackError, PackResult};
pub use index::PackIndex;
pub use manager::{GcReport, PackManager};
pub use reader::PackReader;
pub use writer::{PackFile, PackWriter};

#[cfg(test)]
mod tests {
    use super::*;
    use wll_store::{ObjectKind, StoredObject};
    use wll_types::ObjectId;

    fn make_blob(content: &[u8]) -> StoredObject {
        StoredObject::new(ObjectKind::Blob, content.to_vec())
    }

    #[test]
    fn write_read_roundtrip_single() {
        let blob = make_blob(b"hello world");
        let id = blob.compute_id();

        let mut writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        writer.add_stored_object(&blob);

        let (pack_bytes, index) = writer.finish_to_bytes().unwrap();
        let reader = PackReader::from_bytes(pack_bytes, index).unwrap();

        assert_eq!(reader.object_count(), 1);
        assert!(reader.contains(&id));

        let obj = reader.read_object(&id).unwrap().unwrap();
        assert_eq!(obj.kind, ObjectKind::Blob);
        assert_eq!(obj.data, b"hello world");
    }

    #[test]
    fn write_read_roundtrip_multiple() {
        let objects: Vec<StoredObject> = (0..10)
            .map(|i| make_blob(format!("object-{i}").as_bytes()))
            .collect();
        let ids: Vec<ObjectId> = objects.iter().map(|o| o.compute_id()).collect();

        let mut writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        for obj in &objects {
            writer.add_stored_object(obj);
        }
        assert_eq!(writer.len(), 10);

        let (pack_bytes, index) = writer.finish_to_bytes().unwrap();
        let reader = PackReader::from_bytes(pack_bytes, index).unwrap();

        assert_eq!(reader.object_count(), 10);
        for (i, id) in ids.iter().enumerate() {
            let obj = reader.read_object(id).unwrap().unwrap();
            assert_eq!(obj.data, format!("object-{i}").as_bytes());
        }
    }

    #[test]
    fn write_read_roundtrip_tree() {
        let data = b"tree content";
        let obj = StoredObject::new(ObjectKind::Tree, data.to_vec());
        let id = obj.compute_id();

        let mut writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        writer.add_stored_object(&obj);
        let (bytes, idx) = writer.finish_to_bytes().unwrap();
        let reader = PackReader::from_bytes(bytes, idx).unwrap();

        let read = reader.read_object(&id).unwrap().unwrap();
        assert_eq!(read.kind, ObjectKind::Tree);
        assert_eq!(read.data, data);
    }

    #[test]
    fn empty_pack() {
        let writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        assert!(writer.is_empty());
        let (bytes, idx) = writer.finish_to_bytes().unwrap();
        let reader = PackReader::from_bytes(bytes, idx).unwrap();
        assert_eq!(reader.object_count(), 0);
    }

    #[test]
    fn read_missing_object() {
        let writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        let (bytes, idx) = writer.finish_to_bytes().unwrap();
        let reader = PackReader::from_bytes(bytes, idx).unwrap();
        let result = reader.read_object(&ObjectId::from_bytes(b"missing")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn pack_bad_magic() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(b"BADM");
        let idx = PackIndex::build(vec![], [0u8; 32]);
        let err = PackReader::from_bytes(data, idx).unwrap_err();
        assert!(matches!(err, PackError::InvalidMagic { .. }));
    }

    #[test]
    fn pack_bad_version() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(b"WLLP");
        data[4..8].copy_from_slice(&99u32.to_be_bytes());
        let idx = PackIndex::build(vec![], [0u8; 32]);
        let err = PackReader::from_bytes(data, idx).unwrap_err();
        assert!(matches!(err, PackError::UnsupportedVersion(99)));
    }

    #[test]
    fn pack_too_short() {
        let idx = PackIndex::build(vec![], [0u8; 32]);
        let err = PackReader::from_bytes(vec![1, 2, 3], idx).unwrap_err();
        assert!(matches!(err, PackError::CorruptEntry { .. }));
    }

    #[test]
    fn disk_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pack_base = dir.path().join("test-pack");

        let blob = make_blob(b"disk roundtrip");
        let id = blob.compute_id();

        let mut writer = PackWriter::new(&pack_base);
        writer.add_stored_object(&blob);
        let pack_file = writer.finish().unwrap();

        assert_eq!(pack_file.object_count, 1);
        assert!(pack_file.pack_path.exists());
        assert!(pack_file.index_path.exists());

        let reader = PackReader::open(&pack_file.pack_path).unwrap();
        let obj = reader.read_object(&id).unwrap().unwrap();
        assert_eq!(obj.data, b"disk roundtrip");
    }

    #[test]
    fn large_object_roundtrip() {
        let large_data = vec![0xABu8; 100_000];
        let blob = make_blob(&large_data);
        let id = blob.compute_id();

        let mut writer = PackWriter::new(std::path::Path::new("/tmp/test-pack"));
        writer.add_stored_object(&blob);
        let (bytes, idx) = writer.finish_to_bytes().unwrap();

        // Pack should be smaller than raw data (zstd compression)
        assert!(bytes.len() < large_data.len());

        let reader = PackReader::from_bytes(bytes, idx).unwrap();
        let obj = reader.read_object(&id).unwrap().unwrap();
        assert_eq!(obj.data, large_data);
    }
}
