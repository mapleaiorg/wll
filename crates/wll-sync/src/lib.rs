//! Distributed synchronization for the WorldLine Ledger.
//!
//! Provides push, pull, and fetch operations between WLL repositories.
//! Unlike git, WLL sync also verifies receipt chain integrity on receive.

pub mod error;
pub mod negotiation;
pub mod transport;
pub mod types;
pub mod verifier;

pub use error::{SyncError, SyncResult};
pub use negotiation::NegotiationEngine;
pub use transport::RemoteTransport;
pub use types::{
    CloneOptions, FetchResult, MergeStatus, Negotiation, PullResult, PushResult,
    RefRejection, RefSpec, RefUpdate, VerificationReport,
};
pub use verifier::SyncVerifier;
