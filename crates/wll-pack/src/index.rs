use wll_types::ObjectId;

use crate::error::{PackError, PackResult};

/// Pack index for fast random-access lookups.
///
/// Layout mirrors git's pack index v2:
/// - Fan-out table: 256 entries counting objects with first byte <= index
/// - Sorted ObjectId array
/// - CRC32 array (parallel)
/// - Offset array (parallel)
/// - Pack checksum
#[derive(Clone, Debug)]
pub struct PackIndex {
    pub fan_out: [u32; 256],
    pub object_ids: Vec<ObjectId>,
    pub crc32s: Vec<u32>,
    pub offsets: Vec<u64>,
    pub pack_checksum: [u8; 32],
}

impl PackIndex {
    /// Build an index from (id, crc32, offset) entries and a pack checksum.
    pub fn build(mut entries: Vec<(ObjectId, u32, u64)>, pack_checksum: [u8; 32]) -> Self {
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        let mut fan_out = [0u32; 256];
        let mut object_ids = Vec::with_capacity(entries.len());
        let mut crc32s = Vec::with_capacity(entries.len());
        let mut offsets = Vec::with_capacity(entries.len());

        for (id, crc, offset) in &entries {
            object_ids.push(*id);
            crc32s.push(*crc);
            offsets.push(*offset);
        }

        // Build fan-out: fan_out[i] = count of objects with first byte <= i
        for (i, id) in object_ids.iter().enumerate() {
            let first_byte = id.as_bytes()[0] as usize;
            for slot in first_byte..256 {
                fan_out[slot] = (i + 1) as u32;
            }
        }

        Self {
            fan_out,
            object_ids,
            crc32s,
            offsets,
            pack_checksum,
        }
    }

    /// Look up an object's (offset, crc32) by ID.
    pub fn lookup(&self, id: &ObjectId) -> Option<(u64, u32)> {
        let first_byte = id.as_bytes()[0] as usize;
        let start = if first_byte == 0 {
            0
        } else {
            self.fan_out[first_byte - 1] as usize
        };
        let end = self.fan_out[first_byte] as usize;

        let range = &self.object_ids[start..end];
        match range.binary_search_by(|probe| probe.as_bytes().cmp(id.as_bytes())) {
            Ok(pos) => {
                let idx = start + pos;
                Some((self.offsets[idx], self.crc32s[idx]))
            }
            Err(_) => None,
        }
    }

    /// Total object count.
    pub fn object_count(&self) -> usize {
        self.object_ids.len()
    }

    /// Check if an object exists.
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.lookup(id).is_some()
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> PackResult<Vec<u8>> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"WLLI");
        buf.extend_from_slice(&1u32.to_be_bytes());
        for &count in &self.fan_out {
            buf.extend_from_slice(&count.to_be_bytes());
        }
        for id in &self.object_ids {
            buf.extend_from_slice(id.as_bytes());
        }
        for &crc in &self.crc32s {
            buf.extend_from_slice(&crc.to_be_bytes());
        }
        for &offset in &self.offsets {
            buf.extend_from_slice(&offset.to_be_bytes());
        }
        buf.extend_from_slice(&self.pack_checksum);
        Ok(buf)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> PackResult<Self> {
        if data.len() < 8 {
            return Err(PackError::IndexCorrupted("too short".into()));
        }
        if &data[0..4] != b"WLLI" {
            return Err(PackError::InvalidMagic {
                expected: "WLLI".into(),
                actual: String::from_utf8_lossy(&data[0..4]).into(),
            });
        }
        let version = u32::from_be_bytes(data[4..8].try_into().unwrap());
        if version != 1 {
            return Err(PackError::UnsupportedVersion(version));
        }

        let mut pos = 8;
        if data.len() < pos + 256 * 4 {
            return Err(PackError::IndexCorrupted("fan-out truncated".into()));
        }
        let mut fan_out = [0u32; 256];
        for entry in &mut fan_out {
            *entry = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
            pos += 4;
        }

        let count = fan_out[255] as usize;
        let needed = pos + count * 32 + count * 4 + count * 8 + 32;
        if data.len() < needed {
            return Err(PackError::IndexCorrupted("data truncated".into()));
        }

        let mut object_ids = Vec::with_capacity(count);
        for _ in 0..count {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[pos..pos + 32]);
            object_ids.push(ObjectId::from_hash(hash));
            pos += 32;
        }

        let mut crc32s = Vec::with_capacity(count);
        for _ in 0..count {
            crc32s.push(u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()));
            pos += 4;
        }

        let mut offsets = Vec::with_capacity(count);
        for _ in 0..count {
            offsets.push(u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap()));
            pos += 8;
        }

        let mut pack_checksum = [0u8; 32];
        pack_checksum.copy_from_slice(&data[pos..pos + 32]);

        Ok(Self {
            fan_out,
            object_ids,
            crc32s,
            offsets,
            pack_checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ids(n: usize) -> Vec<ObjectId> {
        (0..n)
            .map(|i| {
                let mut data = vec![0u8; 32];
                data[0] = (i % 256) as u8;
                data[1] = (i / 256) as u8;
                ObjectId::from_hash(data.try_into().unwrap())
            })
            .collect()
    }

    #[test]
    fn build_empty_index() {
        let idx = PackIndex::build(vec![], [0u8; 32]);
        assert_eq!(idx.object_count(), 0);
        assert!(idx.fan_out.iter().all(|&c| c == 0));
    }

    #[test]
    fn build_and_lookup_single() {
        let id = ObjectId::from_bytes(b"hello world test data");
        let entries = vec![(id, 42u32, 100u64)];
        let idx = PackIndex::build(entries, [0u8; 32]);
        assert_eq!(idx.object_count(), 1);
        let (offset, crc) = idx.lookup(&id).unwrap();
        assert_eq!(offset, 100);
        assert_eq!(crc, 42);
    }

    #[test]
    fn lookup_missing_returns_none() {
        let id = ObjectId::from_bytes(b"present");
        let idx = PackIndex::build(vec![(id, 1, 10)], [0u8; 32]);
        let missing = ObjectId::from_bytes(b"missing");
        assert!(idx.lookup(&missing).is_none());
    }

    #[test]
    fn build_and_lookup_multiple() {
        let ids = make_ids(10);
        let entries: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i as u32, (i * 100) as u64))
            .collect();
        let idx = PackIndex::build(entries, [0u8; 32]);
        assert_eq!(idx.object_count(), 10);

        for (i, id) in ids.iter().enumerate() {
            assert!(idx.contains(id), "should contain id {i}");
        }
    }

    #[test]
    fn serialization_roundtrip() {
        let ids = make_ids(5);
        let entries: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, (i * 7) as u32, (i * 50) as u64))
            .collect();
        let checksum = [0xAB; 32];
        let idx = PackIndex::build(entries, checksum);

        let bytes = idx.to_bytes().unwrap();
        let idx2 = PackIndex::from_bytes(&bytes).unwrap();

        assert_eq!(idx2.object_count(), idx.object_count());
        assert_eq!(idx2.pack_checksum, checksum);

        for id in &ids {
            assert_eq!(idx.lookup(id), idx2.lookup(id));
        }
    }

    #[test]
    fn from_bytes_bad_magic() {
        let err = PackIndex::from_bytes(b"BADMxxxxxxxx").unwrap_err();
        assert!(matches!(err, PackError::InvalidMagic { .. }));
    }

    #[test]
    fn from_bytes_bad_version() {
        let mut data = Vec::new();
        data.extend_from_slice(b"WLLI");
        data.extend_from_slice(&99u32.to_be_bytes());
        let err = PackIndex::from_bytes(&data).unwrap_err();
        assert!(matches!(err, PackError::UnsupportedVersion(99)));
    }

    #[test]
    fn from_bytes_truncated() {
        let err = PackIndex::from_bytes(b"WLLI").unwrap_err();
        assert!(matches!(err, PackError::IndexCorrupted(_)));
    }

    #[test]
    fn contains_works() {
        let id = ObjectId::from_bytes(b"test");
        let idx = PackIndex::build(vec![(id, 1, 0)], [0u8; 32]);
        assert!(idx.contains(&id));
        assert!(!idx.contains(&ObjectId::null()));
    }
}
