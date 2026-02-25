use std::path::{Path, PathBuf};

use wll_store::{ObjectKind, StoredObject};
use wll_types::ObjectId;

use crate::entry::{PackEntry, PackObjectKind};
use crate::error::{PackError, PackResult};
use crate::index::PackIndex;

/// Result of writing a pack file.
#[derive(Clone, Debug)]
pub struct PackFile {
    pub pack_path: PathBuf,
    pub index_path: PathBuf,
    pub object_count: usize,
    pub checksum: [u8; 32],
}

/// Builds a pack file from a collection of objects.
pub struct PackWriter {
    path: PathBuf,
    entries: Vec<PackEntry>,
}

impl PackWriter {
    /// Create a new PackWriter targeting the given base path.
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            entries: Vec::new(),
        }
    }

    /// Add an object to the pack.
    pub fn add_object(&mut self, id: ObjectId, kind: ObjectKind, data: &[u8]) {
        self.entries.push(PackEntry {
            id,
            kind: PackObjectKind::Full(kind),
            data: data.to_vec(),
            crc32: 0, // computed at write time
        });
    }

    /// Add a stored object directly.
    pub fn add_stored_object(&mut self, obj: &StoredObject) {
        let id = obj.compute_id();
        self.add_object(id, obj.kind, &obj.data);
    }

    /// Number of objects queued.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Write the pack and index to disk.
    pub fn finish(self) -> PackResult<PackFile> {
        let pack_path = self.path.with_extension("pack");
        let index_path = self.path.with_extension("idx");

        let (pack_data, index) = self.build_pack_bytes()?;

        std::fs::write(&pack_path, &pack_data)?;
        std::fs::write(&index_path, &index.to_bytes()?)?;

        Ok(PackFile {
            pack_path,
            index_path,
            object_count: index.object_count(),
            checksum: index.pack_checksum,
        })
    }

    /// Build pack bytes and index in memory (no disk I/O).
    pub fn finish_to_bytes(self) -> PackResult<(Vec<u8>, PackIndex)> {
        self.build_pack_bytes()
    }

    fn build_pack_bytes(self) -> PackResult<(Vec<u8>, PackIndex)> {
        let mut pack_data = Vec::new();
        let mut index_entries = Vec::new();

        // Header: magic + version + object count
        pack_data.extend_from_slice(b"WLLP");
        pack_data.extend_from_slice(&1u32.to_be_bytes());
        pack_data.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());

        for entry in &self.entries {
            let offset = pack_data.len() as u64;

            // Type byte
            pack_data.push(entry.kind.type_byte());

            // Compress data
            let compressed = zstd::encode_all(entry.data.as_slice(), 3)
                .map_err(|e| PackError::CompressionFailed(e.to_string()))?;

            // Varint: uncompressed size
            encode_varint(&mut pack_data, entry.data.len() as u64);
            // Varint: compressed size
            encode_varint(&mut pack_data, compressed.len() as u64);

            // If delta, write base ID (not used yet but format supports it)
            if let PackObjectKind::Delta { base } = &entry.kind {
                pack_data.extend_from_slice(base.as_bytes());
            }

            let crc = crc32fast::hash(&compressed);
            pack_data.extend_from_slice(&compressed);

            index_entries.push((entry.id, crc, offset));
        }

        // Pack trailer: BLAKE3 checksum of everything so far
        let checksum = *blake3::hash(&pack_data).as_bytes();
        pack_data.extend_from_slice(&checksum);

        let index = PackIndex::build(index_entries, checksum);
        Ok((pack_data, index))
    }
}

/// Encode a u64 as a variable-length integer.
pub(crate) fn encode_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Decode a variable-length integer. Returns (value, bytes_consumed).
pub(crate) fn decode_varint(data: &[u8]) -> PackResult<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        value |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
        if shift >= 64 {
            return Err(PackError::CorruptEntry {
                offset: 0,
                reason: "varint overflow".into(),
            });
        }
    }
    Err(PackError::CorruptEntry {
        offset: 0,
        reason: "truncated varint".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip_small() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 42);
        let (val, consumed) = decode_varint(&buf).unwrap();
        assert_eq!(val, 42);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn varint_roundtrip_large() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 1_000_000);
        let (val, _) = decode_varint(&buf).unwrap();
        assert_eq!(val, 1_000_000);
    }

    #[test]
    fn varint_zero() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 0);
        let (val, consumed) = decode_varint(&buf).unwrap();
        assert_eq!(val, 0);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn varint_max_u64() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, u64::MAX);
        let (val, _) = decode_varint(&buf).unwrap();
        assert_eq!(val, u64::MAX);
    }

    #[test]
    fn decode_varint_truncated() {
        let err = decode_varint(&[0x80]).unwrap_err();
        assert!(matches!(err, PackError::CorruptEntry { .. }));
    }
}
