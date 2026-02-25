use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::TypeError;

/// Content-addressed identifier for any stored object.
///
/// An `ObjectId` is the BLAKE3 hash of an object's content. Identical content
/// always produces the same `ObjectId`, making objects deduplicatable and
/// verifiable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId([u8; 32]);

impl ObjectId {
    /// Compute an `ObjectId` from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// Create an `ObjectId` from a pre-computed hash.
    pub fn from_hash(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    /// The null object ID (all zeros). Represents "no object".
    pub const fn null() -> Self {
        Self([0u8; 32])
    }

    /// Returns `true` if this is the null object ID.
    pub fn is_null(&self) -> bool {
        self.0 == [0u8; 32]
    }

    /// The raw 32-byte hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Hex-encoded string representation.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Short hex representation (first 8 characters).
    pub fn short_hex(&self) -> String {
        hex::encode(&self.0[..4])
    }

    /// Parse from a hex string.
    pub fn from_hex(s: &str) -> Result<Self, TypeError> {
        let bytes = hex::decode(s).map_err(|e| TypeError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(TypeError::InvalidLength {
                expected: 32,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", self.short_hex())
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl From<[u8; 32]> for ObjectId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<ObjectId> for [u8; 32] {
    fn from(id: ObjectId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_is_deterministic() {
        let data = b"hello world";
        let id1 = ObjectId::from_bytes(data);
        let id2 = ObjectId::from_bytes(data);
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_data_produces_different_ids() {
        let id1 = ObjectId::from_bytes(b"hello");
        let id2 = ObjectId::from_bytes(b"world");
        assert_ne!(id1, id2);
    }

    #[test]
    fn null_is_all_zeros() {
        let null = ObjectId::null();
        assert!(null.is_null());
        assert_eq!(null.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn hex_roundtrip() {
        let id = ObjectId::from_bytes(b"test");
        let hex = id.to_hex();
        let parsed = ObjectId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn short_hex_is_8_chars() {
        let id = ObjectId::from_bytes(b"test");
        assert_eq!(id.short_hex().len(), 8);
    }

    #[test]
    fn display_is_full_hex() {
        let id = ObjectId::from_bytes(b"test");
        let display = format!("{id}");
        assert_eq!(display.len(), 64);
        assert_eq!(display, id.to_hex());
    }

    #[test]
    fn serde_roundtrip() {
        let id = ObjectId::from_bytes(b"serde test");
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ObjectId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn ordering_is_consistent() {
        let id1 = ObjectId::from_hash([0; 32]);
        let id2 = ObjectId::from_hash([1; 32]);
        assert!(id1 < id2);
    }
}
