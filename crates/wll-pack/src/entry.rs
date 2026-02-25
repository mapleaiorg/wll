use serde::{Deserialize, Serialize};
use wll_store::ObjectKind;
use wll_types::ObjectId;

/// Type tag for pack entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackObjectKind {
    /// Complete object with its full data.
    Full(ObjectKind),
    /// Delta-compressed against a base object.
    Delta { base: ObjectId },
}

impl PackObjectKind {
    /// Serialize to a type byte for the pack format.
    pub fn type_byte(&self) -> u8 {
        match self {
            Self::Full(ObjectKind::Blob) => 1,
            Self::Full(ObjectKind::Tree) => 2,
            Self::Full(ObjectKind::Receipt) => 3,
            Self::Full(ObjectKind::Snapshot) => 4,
            Self::Full(ObjectKind::Pack) => 5,
            Self::Delta { .. } => 6,
        }
    }

    /// Parse from a type byte (full objects only; deltas need the base ID).
    pub fn from_type_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::Full(ObjectKind::Blob)),
            2 => Some(Self::Full(ObjectKind::Tree)),
            3 => Some(Self::Full(ObjectKind::Receipt)),
            4 => Some(Self::Full(ObjectKind::Snapshot)),
            5 => Some(Self::Full(ObjectKind::Pack)),
            _ => None,
        }
    }
}

/// A single entry in a pack file.
#[derive(Clone, Debug)]
pub struct PackEntry {
    /// Content-addressed ID of the object.
    pub id: ObjectId,
    /// Type of this pack entry.
    pub kind: PackObjectKind,
    /// Uncompressed data.
    pub data: Vec<u8>,
    /// CRC32 checksum of the compressed data.
    pub crc32: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_byte_roundtrip_blob() {
        let kind = PackObjectKind::Full(ObjectKind::Blob);
        let byte = kind.type_byte();
        assert_eq!(byte, 1);
        assert_eq!(PackObjectKind::from_type_byte(byte), Some(kind));
    }

    #[test]
    fn type_byte_roundtrip_tree() {
        let kind = PackObjectKind::Full(ObjectKind::Tree);
        assert_eq!(kind.type_byte(), 2);
        assert_eq!(PackObjectKind::from_type_byte(2), Some(kind));
    }

    #[test]
    fn type_byte_roundtrip_receipt() {
        let kind = PackObjectKind::Full(ObjectKind::Receipt);
        assert_eq!(kind.type_byte(), 3);
        assert_eq!(PackObjectKind::from_type_byte(3), Some(kind));
    }

    #[test]
    fn type_byte_roundtrip_snapshot() {
        let kind = PackObjectKind::Full(ObjectKind::Snapshot);
        assert_eq!(kind.type_byte(), 4);
        assert_eq!(PackObjectKind::from_type_byte(4), Some(kind));
    }

    #[test]
    fn type_byte_delta() {
        let kind = PackObjectKind::Delta {
            base: ObjectId::null(),
        };
        assert_eq!(kind.type_byte(), 6);
    }

    #[test]
    fn from_type_byte_unknown() {
        assert!(PackObjectKind::from_type_byte(0).is_none());
        assert!(PackObjectKind::from_type_byte(7).is_none());
        assert!(PackObjectKind::from_type_byte(255).is_none());
    }
}
