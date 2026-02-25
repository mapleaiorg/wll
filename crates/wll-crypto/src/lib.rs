//! Cryptographic primitives for the WorldLine Ledger.
//!
//! Provides domain-separated BLAKE3 hashing, Ed25519 signing/verification,
//! binary Merkle trees with inclusion proofs, and hash chain verification.
//!
//! All crypto operations wrap established libraries — no custom cryptography.

pub mod chain;
pub mod hasher;
pub mod merkle;
pub mod signer;

pub use chain::{HasReceiptHash, HashChainVerifier};
pub use hasher::ContentHasher;
pub use merkle::{MerkleProof, MerkleTree, Side};
pub use signer::{Signature, SigningKey, VerifyingKey};
