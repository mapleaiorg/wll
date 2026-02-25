use wll_store::StoredObject;
use wll_types::ObjectId;

use crate::entry::PackObjectKind;
use crate::error::{PackError, PackResult};
use crate::index::PackIndex;
use crate::writer::decode_varint;

/// Reads objects from a pack file using an index for random access.
#[derive(Debug)]
pub struct PackReader {
    pack_data: Vec<u8>,
    index: PackIndex,
}

impl PackReader {
    /// Open from raw bytes.
    pub fn from_bytes(pack_data: Vec<u8>, index: PackIndex) -> PackResult<Self> {
        if pack_data.len() < 12 {
            return Err(PackError::CorruptEntry {
                offset: 0,
                reason: "pack data too short".into(),
            });
        }
        if &pack_data[0..4] != b"WLLP" {
            return Err(PackError::InvalidMagic {
                expected: "WLLP".into(),
                actual: String::from_utf8_lossy(&pack_data[0..4]).into(),
            });
        }
        let version = u32::from_be_bytes(pack_data[4..8].try_into().unwrap());
        if version != 1 {
            return Err(PackError::UnsupportedVersion(version));
        }
        Ok(Self { pack_data, index })
    }

    /// Open from disk paths.
    pub fn open(pack_path: &std::path::Path) -> PackResult<Self> {
        let pack_data = std::fs::read(pack_path)?;
        let index_path = pack_path.with_extension("idx");
        let index_data = std::fs::read(&index_path)?;
        let index = PackIndex::from_bytes(&index_data)?;
        Self::from_bytes(pack_data, index)
    }

    /// Read an object by ID.
    pub fn read_object(&self, id: &ObjectId) -> PackResult<Option<StoredObject>> {
        let (offset, expected_crc) = match self.index.lookup(id) {
            Some(v) => v,
            None => return Ok(None),
        };
        let obj = self.read_at_offset(offset, expected_crc)?;
        Ok(Some(obj))
    }

    /// Check containment.
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.index.contains(id)
    }

    /// Object count.
    pub fn object_count(&self) -> usize {
        self.index.object_count()
    }

    /// Access the index.
    pub fn index(&self) -> &PackIndex {
        &self.index
    }

    /// List all object IDs.
    pub fn object_ids(&self) -> &[ObjectId] {
        &self.index.object_ids
    }

    fn read_at_offset(&self, offset: u64, expected_crc: u32) -> PackResult<StoredObject> {
        let data = &self.pack_data;
        let mut pos = offset as usize;

        if pos >= data.len() {
            return Err(PackError::CorruptEntry {
                offset,
                reason: "offset beyond pack data".into(),
            });
        }

        let type_byte = data[pos];
        pos += 1;

        let kind = PackObjectKind::from_type_byte(type_byte).ok_or_else(|| {
            PackError::CorruptEntry {
                offset,
                reason: format!("unknown type byte: {type_byte}"),
            }
        })?;

        let (uncompressed_size, consumed) = decode_varint(&data[pos..])?;
        pos += consumed;

        let (compressed_size, consumed) = decode_varint(&data[pos..])?;
        pos += consumed;

        let end = pos + compressed_size as usize;
        if end > data.len() {
            return Err(PackError::CorruptEntry {
                offset,
                reason: "compressed data extends beyond pack".into(),
            });
        }
        let compressed = &data[pos..end];

        let actual_crc = crc32fast::hash(compressed);
        if actual_crc != expected_crc {
            return Err(PackError::CrcMismatch {
                id: ObjectId::null(),
            });
        }

        let decompressed = zstd::decode_all(compressed)
            .map_err(|e| PackError::DecompressionFailed(e.to_string()))?;

        if decompressed.len() != uncompressed_size as usize {
            return Err(PackError::CorruptEntry {
                offset,
                reason: format!(
                    "size mismatch: expected {uncompressed_size}, got {}",
                    decompressed.len()
                ),
            });
        }

        let object_kind = match kind {
            PackObjectKind::Full(k) => k,
            PackObjectKind::Delta { .. } => {
                return Err(PackError::CorruptEntry {
                    offset,
                    reason: "delta resolution not supported".into(),
                });
            }
        };

        Ok(StoredObject::new(object_kind, decompressed))
    }
}
