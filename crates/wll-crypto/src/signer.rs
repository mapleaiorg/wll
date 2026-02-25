use serde::{Deserialize, Serialize};
use wll_types::WorldlineId;
use wll_types::identity::IdentityMaterial;

/// Ed25519 signing key (private).
pub struct SigningKey(ed25519_dalek::SigningKey);

/// Ed25519 verifying key (public).
#[derive(Clone, PartialEq, Eq)]
pub struct VerifyingKey(ed25519_dalek::VerifyingKey);

/// Ed25519 signature.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "signature_serde")] ed25519_dalek::Signature);

impl SigningKey {
    /// Generate a new random signing key.
    pub fn generate() -> Self {
        let mut csprng = rand::thread_rng();
        Self(ed25519_dalek::SigningKey::generate(&mut csprng))
    }

    /// Create from raw 32-byte secret.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(&bytes))
    }

    /// The corresponding public verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key())
    }

    /// Sign a message.
    pub fn sign(&self, message: &[u8]) -> Signature {
        use ed25519_dalek::Signer;
        Signature(self.0.sign(message))
    }

    /// Raw secret key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl VerifyingKey {
    /// Verify a signature on a message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), SignatureError> {
        use ed25519_dalek::Verifier;
        self.0
            .verify(message, &signature.0)
            .map_err(|_| SignatureError::InvalidSignature)
    }

    /// Derive a WorldlineId from this public key.
    pub fn to_worldline_id(&self) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::PublicKey(self.0.to_bytes()))
    }

    /// Raw public key bytes.
    pub fn as_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Create from raw 32-byte public key.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SignatureError> {
        let key = ed25519_dalek::VerifyingKey::from_bytes(&bytes)
            .map_err(|_| SignatureError::InvalidKey)?;
        Ok(Self(key))
    }
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SigningKey(<redacted>)")
    }
}

impl std::fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VerifyingKey({})", hex::encode(self.0.to_bytes()))
    }
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signature({}...)",
            hex::encode(&self.0.to_bytes()[..8])
        )
    }
}

/// Errors from signing operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SignatureError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid key")]
    InvalidKey,
}

mod signature_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(sig: &ed25519_dalek::Signature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&sig.to_bytes())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ed25519_dalek::Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        let arr: [u8; 64] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 64-byte signature"))?;
        Ok(ed25519_dalek::Signature::from_bytes(&arr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let sk = SigningKey::generate();
        let vk = sk.verifying_key();
        let message = b"hello world";
        let sig = sk.sign(message);
        assert!(vk.verify(message, &sig).is_ok());
    }

    #[test]
    fn verify_fails_on_wrong_message() {
        let sk = SigningKey::generate();
        let vk = sk.verifying_key();
        let sig = sk.sign(b"correct message");
        assert!(vk.verify(b"wrong message", &sig).is_err());
    }

    #[test]
    fn verify_fails_with_wrong_key() {
        let sk1 = SigningKey::generate();
        let sk2 = SigningKey::generate();
        let sig = sk1.sign(b"message");
        assert!(sk2.verifying_key().verify(b"message", &sig).is_err());
    }

    #[test]
    fn worldline_id_from_key() {
        let sk = SigningKey::generate();
        let vk = sk.verifying_key();
        let wid1 = vk.to_worldline_id();
        let wid2 = vk.to_worldline_id();
        assert_eq!(wid1, wid2); // deterministic
    }

    #[test]
    fn different_keys_different_worldlines() {
        let sk1 = SigningKey::generate();
        let sk2 = SigningKey::generate();
        let wid1 = sk1.verifying_key().to_worldline_id();
        let wid2 = sk2.verifying_key().to_worldline_id();
        assert_ne!(wid1, wid2);
    }

    #[test]
    fn from_bytes_roundtrip() {
        let sk = SigningKey::generate();
        let bytes = *sk.as_bytes();
        let sk2 = SigningKey::from_bytes(bytes);
        assert_eq!(sk.verifying_key(), sk2.verifying_key());
    }

    #[test]
    fn signature_serde_roundtrip() {
        let sk = SigningKey::generate();
        let sig = sk.sign(b"test");
        let json = serde_json::to_string(&sig).unwrap();
        let parsed: Signature = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, parsed);
    }

    #[test]
    fn debug_redacts_signing_key() {
        let sk = SigningKey::generate();
        let debug = format!("{sk:?}");
        assert!(debug.contains("redacted"));
    }
}
