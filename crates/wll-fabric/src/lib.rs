//! Event fabric for the WorldLine Ledger.
//!
//! Provides crash-recoverable event routing with a Write-Ahead Log (WAL) and
//! Hybrid Logical Clock (HLC) for causal ordering. This is the real-time
//! event infrastructure that drives receipt creation.

pub mod error;
pub mod event;
pub mod fabric;
pub mod hlc;
pub mod wal;

pub use error::FabricError;
pub use event::{EventKind, EventPayload, FabricEvent};
pub use fabric::{EventFabric, EventFilter};
pub use hlc::HybridLogicalClock;
pub use wal::{SyncMode, WalConfig, WriteAheadLog};
