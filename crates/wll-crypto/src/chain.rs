/// Trait for objects that participate in a hash chain.
pub trait HasReceiptHash {
    /// The receipt's own hash.
    fn receipt_hash(&self) -> [u8; 32];
    /// The previous receipt's hash (None for genesis).
    fn prev_hash(&self) -> Option<[u8; 32]>;
    /// Canonical payload bytes for hash verification.
    fn payload_bytes(&self) -> Vec<u8>;
}

/// Hash chain integrity verifier.
///
/// Verifies that a sequence of receipts forms a valid hash chain:
/// each receipt's prev_hash matches the previous receipt's receipt_hash,
/// and each receipt's hash is correctly computed from its payload.
pub struct HashChainVerifier;

impl HashChainVerifier {
    /// Verify a chain of receipts.
    ///
    /// Checks:
    /// 1. First receipt has no previous hash
    /// 2. Each subsequent receipt's prev_hash matches the previous receipt_hash
    /// 3. Each receipt's hash is correct for its payload
    pub fn verify_chain(receipts: &[impl HasReceiptHash]) -> Result<(), ChainError> {
        if receipts.is_empty() {
            return Ok(());
        }

        // First receipt must have no previous hash
        if receipts[0].prev_hash().is_some() {
            return Err(ChainError::GenesisHasPrevHash);
        }

        // Verify first receipt hash
        let computed = Self::compute_hash(&receipts[0].payload_bytes(), None);
        if computed != receipts[0].receipt_hash() {
            return Err(ChainError::HashMismatch { index: 0 });
        }

        // Verify chain links
        for i in 1..receipts.len() {
            let expected_prev = receipts[i - 1].receipt_hash();
            match receipts[i].prev_hash() {
                Some(prev) if prev == expected_prev => {}
                Some(_) => return Err(ChainError::BrokenLink { index: i }),
                None => return Err(ChainError::MissingPrevHash { index: i }),
            }

            let computed =
                Self::compute_hash(&receipts[i].payload_bytes(), Some(expected_prev));
            if computed != receipts[i].receipt_hash() {
                return Err(ChainError::HashMismatch { index: i });
            }
        }

        Ok(())
    }

    /// Compute the expected hash for a receipt payload and optional previous hash.
    pub fn compute_hash(payload: &[u8], prev_hash: Option<[u8; 32]>) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"wll-receipt-v1:");
        if let Some(prev) = prev_hash {
            hasher.update(&prev);
        }
        hasher.update(payload);
        *hasher.finalize().as_bytes()
    }
}

/// Errors from chain verification.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("genesis receipt has a previous hash (should be None)")]
    GenesisHasPrevHash,

    #[error("broken link at index {index}: prev_hash does not match")]
    BrokenLink { index: usize },

    #[error("missing prev_hash at index {index} (should reference previous receipt)")]
    MissingPrevHash { index: usize },

    #[error("hash mismatch at index {index}: computed hash differs from stored")]
    HashMismatch { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test receipt for chain verification.
    struct TestReceipt {
        hash: [u8; 32],
        prev: Option<[u8; 32]>,
        payload: Vec<u8>,
    }

    impl HasReceiptHash for TestReceipt {
        fn receipt_hash(&self) -> [u8; 32] {
            self.hash
        }
        fn prev_hash(&self) -> Option<[u8; 32]> {
            self.prev
        }
        fn payload_bytes(&self) -> Vec<u8> {
            self.payload.clone()
        }
    }

    fn build_chain(count: usize) -> Vec<TestReceipt> {
        let mut chain = Vec::new();
        let mut prev_hash: Option<[u8; 32]> = None;

        for i in 0..count {
            let payload = format!("receipt-{i}").into_bytes();
            let hash = HashChainVerifier::compute_hash(&payload, prev_hash);
            chain.push(TestReceipt {
                hash,
                prev: prev_hash,
                payload,
            });
            prev_hash = Some(hash);
        }

        chain
    }

    #[test]
    fn empty_chain_is_valid() {
        let chain: Vec<TestReceipt> = vec![];
        assert!(HashChainVerifier::verify_chain(&chain).is_ok());
    }

    #[test]
    fn single_receipt_chain() {
        let chain = build_chain(1);
        assert!(HashChainVerifier::verify_chain(&chain).is_ok());
    }

    #[test]
    fn multi_receipt_chain() {
        let chain = build_chain(10);
        assert!(HashChainVerifier::verify_chain(&chain).is_ok());
    }

    #[test]
    fn genesis_with_prev_hash_fails() {
        let mut chain = build_chain(1);
        chain[0].prev = Some([1; 32]);
        let err = HashChainVerifier::verify_chain(&chain).unwrap_err();
        assert_eq!(err, ChainError::GenesisHasPrevHash);
    }

    #[test]
    fn broken_link_detected() {
        let mut chain = build_chain(3);
        chain[2].prev = Some([99; 32]); // wrong prev hash
        let err = HashChainVerifier::verify_chain(&chain).unwrap_err();
        assert_eq!(err, ChainError::BrokenLink { index: 2 });
    }

    #[test]
    fn missing_prev_hash_detected() {
        let mut chain = build_chain(3);
        chain[1].prev = None; // should have prev
        let err = HashChainVerifier::verify_chain(&chain).unwrap_err();
        assert_eq!(err, ChainError::MissingPrevHash { index: 1 });
    }

    #[test]
    fn tampered_payload_detected() {
        let mut chain = build_chain(3);
        chain[1].payload = b"tampered".to_vec(); // change payload without updating hash
        let err = HashChainVerifier::verify_chain(&chain).unwrap_err();
        assert_eq!(err, ChainError::HashMismatch { index: 1 });
    }
}
