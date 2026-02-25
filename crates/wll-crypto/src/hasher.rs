use wll_types::ObjectId;

/// Domain-separated BLAKE3 content hasher.
///
/// Each hasher carries a domain tag (e.g., `"wll-blob-v1"`, `"wll-receipt-v1"`)
/// that is prepended to every hash computation. This prevents cross-type hash
/// collisions: a blob and a receipt with identical bytes will produce different
/// hashes.
pub struct ContentHasher {
    domain: &'static str,
}

impl ContentHasher {
    /// Hasher for blob objects.
    pub const BLOB: Self = Self {
        domain: "wll-blob-v1",
    };
    /// Hasher for tree objects.
    pub const TREE: Self = Self {
        domain: "wll-tree-v1",
    };
    /// Hasher for receipt objects.
    pub const RECEIPT: Self = Self {
        domain: "wll-receipt-v1",
    };
    /// Hasher for commit/snapshot objects.
    pub const COMMIT: Self = Self {
        domain: "wll-commit-v1",
    };

    /// Create a hasher with a custom domain tag.
    pub const fn new(domain: &'static str) -> Self {
        Self { domain }
    }

    /// Hash raw bytes with domain separation.
    pub fn hash(&self, data: &[u8]) -> ObjectId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.domain.as_bytes());
        hasher.update(b":");
        hasher.update(data);
        ObjectId::from_hash(*hasher.finalize().as_bytes())
    }

    /// Hash a serializable value as JSON with domain separation.
    pub fn hash_json<T: serde::Serialize>(&self, value: &T) -> Result<ObjectId, HasherError> {
        let data = serde_json::to_vec(value).map_err(|e| HasherError::Serialization(e.to_string()))?;
        Ok(self.hash(&data))
    }

    /// Verify that data produces the expected object ID.
    pub fn verify(&self, data: &[u8], expected: &ObjectId) -> bool {
        self.hash(data) == *expected
    }

    /// Raw BLAKE3 hash without domain separation (for low-level use).
    pub fn raw_hash(data: &[u8]) -> [u8; 32] {
        *blake3::hash(data).as_bytes()
    }

    /// The domain tag used by this hasher.
    pub fn domain(&self) -> &str {
        self.domain
    }
}

/// Errors from hashing operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HasherError {
    #[error("serialization error: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let data = b"hello world";
        let id1 = ContentHasher::BLOB.hash(data);
        let id2 = ContentHasher::BLOB.hash(data);
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_domains_produce_different_hashes() {
        let data = b"same content";
        let blob_hash = ContentHasher::BLOB.hash(data);
        let tree_hash = ContentHasher::TREE.hash(data);
        let receipt_hash = ContentHasher::RECEIPT.hash(data);
        assert_ne!(blob_hash, tree_hash);
        assert_ne!(blob_hash, receipt_hash);
        assert_ne!(tree_hash, receipt_hash);
    }

    #[test]
    fn verify_correct_data() {
        let data = b"test data";
        let id = ContentHasher::BLOB.hash(data);
        assert!(ContentHasher::BLOB.verify(data, &id));
    }

    #[test]
    fn verify_incorrect_data() {
        let id = ContentHasher::BLOB.hash(b"original");
        assert!(!ContentHasher::BLOB.verify(b"tampered", &id));
    }

    #[test]
    fn hash_json_works() {
        let value = serde_json::json!({"key": "value", "num": 42});
        let id = ContentHasher::COMMIT.hash_json(&value).unwrap();
        assert!(!id.is_null());
    }

    #[test]
    fn custom_domain() {
        let hasher = ContentHasher::new("my-custom-domain-v1");
        let id = hasher.hash(b"data");
        assert_ne!(id, ContentHasher::BLOB.hash(b"data"));
    }

    #[test]
    fn raw_hash_no_domain() {
        let h1 = ContentHasher::raw_hash(b"test");
        let h2 = ContentHasher::raw_hash(b"test");
        assert_eq!(h1, h2);
        // Raw hash should differ from domain-separated hash
        let domain_hash = ContentHasher::BLOB.hash(b"test");
        assert_ne!(h1, *domain_hash.as_bytes());
    }
}
