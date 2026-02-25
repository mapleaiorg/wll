//! Foundation types for the WorldLine Ledger (WLL).
//!
//! This crate provides the core identity, temporal, and structural types used
//! throughout the WLL system. Every other WLL crate depends on `wll-types`.
//!
//! # Key Types
//!
//! - [`WorldlineId`] — Persistent cryptographic identity derived from genesis material
//! - [`ObjectId`] — Content-addressed identifier (BLAKE3 hash)
//! - [`TemporalAnchor`] — Hybrid Logical Clock timestamp for causal ordering
//! - [`CommitmentId`] — UUID v7 commitment identifier
//! - [`CommitmentClass`] — Risk classification for policy gating
//! - [`Decision`] — Policy evaluation result
//! - [`EvidenceBundle`] — External evidence references

pub mod commitment;
pub mod error;
pub mod evidence;
pub mod identity;
pub mod object;
pub mod receipt;
pub mod temporal;

pub use commitment::{
    Capability, CapabilityId, CapabilityScope, CommitmentClass, CommitmentId, Reversibility,
};
pub use error::TypeError;
pub use evidence::EvidenceBundle;
pub use identity::{IdentityMaterial, WorldlineId};
pub use object::ObjectId;
pub use receipt::{ReceiptId, ReceiptKind};
pub use temporal::TemporalAnchor;
