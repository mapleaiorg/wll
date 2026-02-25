use std::fmt;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::error::TypeError;

/// Material used to derive a [`WorldlineId`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityMaterial {
    /// Genesis from a raw 32-byte hash (e.g. initial content hash).
    GenesisHash([u8; 32]),
    /// Genesis from an ed25519 public key (32 bytes).
    PublicKey([u8; 32]),
    /// Derived identity from a parent worldline and a label.
    Derived { parent: [u8; 32], label: String },
}

/// Persistent cryptographic identity for a worldline.
///
/// A `WorldlineId` is derived deterministically from [`IdentityMaterial`]
/// using BLAKE3. The same material always produces the same identity.
/// WorldlineIds are the fundamental identity primitive in WLL — they
/// persist across time and are unforgeable.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorldlineId {
    hash: [u8; 32],
}

impl WorldlineId {
    /// Derive a `WorldlineId` from identity material.
    pub fn derive(material: &IdentityMaterial) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"wll-worldline-v1:");
        match material {
            IdentityMaterial::GenesisHash(h) => {
                hasher.update(b"genesis:");
                hasher.update(h);
            }
            IdentityMaterial::PublicKey(pk) => {
                hasher.update(b"pubkey:");
                hasher.update(pk);
            }
            IdentityMaterial::Derived { parent, label } => {
                hasher.update(b"derived:");
                hasher.update(parent);
                hasher.update(b":");
                hasher.update(label.as_bytes());
            }
        }
        Self {
            hash: *hasher.finalize().as_bytes(),
        }
    }

    /// Create an ephemeral (random) WorldlineId for tests and demos.
    pub fn ephemeral() -> Self {
        let mut bytes = [0u8; 32];
        rand::Rng::fill(&mut rand::thread_rng(), &mut bytes);
        Self::derive(&IdentityMaterial::GenesisHash(bytes))
    }

    /// The raw 32-byte hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Full hex-encoded string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.hash)
    }

    /// Short identifier (first 8 hex characters).
    pub fn short_id(&self) -> String {
        format!("wl:{}", hex::encode(&self.hash[..4]))
    }

    /// Parse from a hex string (64 hex characters).
    pub fn from_hex(s: &str) -> Result<Self, TypeError> {
        let s = s.strip_prefix("wl:").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| TypeError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(TypeError::InvalidLength {
                expected: 32,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self { hash: arr })
    }

    /// Create from a raw 32-byte hash. Use `derive()` for production code.
    pub fn from_raw(hash: [u8; 32]) -> Self {
        Self { hash }
    }
}

impl fmt::Debug for WorldlineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorldlineId({})", self.short_id())
    }
}

impl fmt::Display for WorldlineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic() {
        let material = IdentityMaterial::GenesisHash([42u8; 32]);
        let id1 = WorldlineId::derive(&material);
        let id2 = WorldlineId::derive(&material);
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_material_produces_different_ids() {
        let id1 = WorldlineId::derive(&IdentityMaterial::GenesisHash([1; 32]));
        let id2 = WorldlineId::derive(&IdentityMaterial::GenesisHash([2; 32]));
        assert_ne!(id1, id2);
    }

    #[test]
    fn different_material_types_produce_different_ids() {
        let bytes = [7u8; 32];
        let genesis = WorldlineId::derive(&IdentityMaterial::GenesisHash(bytes));
        let pubkey = WorldlineId::derive(&IdentityMaterial::PublicKey(bytes));
        assert_ne!(genesis, pubkey);
    }

    #[test]
    fn derived_identity_includes_label() {
        let parent = [5u8; 32];
        let id1 = WorldlineId::derive(&IdentityMaterial::Derived {
            parent,
            label: "child-a".into(),
        });
        let id2 = WorldlineId::derive(&IdentityMaterial::Derived {
            parent,
            label: "child-b".into(),
        });
        assert_ne!(id1, id2);
    }

    #[test]
    fn ephemeral_ids_are_unique() {
        let id1 = WorldlineId::ephemeral();
        let id2 = WorldlineId::ephemeral();
        assert_ne!(id1, id2);
    }

    #[test]
    fn short_id_format() {
        let id = WorldlineId::derive(&IdentityMaterial::GenesisHash([0; 32]));
        let short = id.short_id();
        assert!(short.starts_with("wl:"));
        assert_eq!(short.len(), 11); // "wl:" + 8 hex chars
    }

    #[test]
    fn hex_roundtrip() {
        let id = WorldlineId::derive(&IdentityMaterial::GenesisHash([99; 32]));
        let hex = id.to_hex();
        let parsed = WorldlineId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn hex_roundtrip_with_prefix() {
        let id = WorldlineId::derive(&IdentityMaterial::GenesisHash([99; 32]));
        let prefixed = format!("wl:{}", id.to_hex());
        let parsed = WorldlineId::from_hex(&prefixed).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_roundtrip() {
        let id = WorldlineId::derive(&IdentityMaterial::GenesisHash([10; 32]));
        let json = serde_json::to_string(&id).unwrap();
        let parsed: WorldlineId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn ordering_is_consistent() {
        let id1 = WorldlineId::from_raw([0; 32]);
        let id2 = WorldlineId::from_raw([1; 32]);
        assert!(id1 < id2);
    }
}
