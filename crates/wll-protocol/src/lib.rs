//! Wire protocol for the WorldLine Ledger.
//!
//! Defines the framing, message types, and serialization format used between
//! WLL clients and servers during sync, push, pull, and query operations.

pub mod auth;
pub mod codec;
pub mod endpoint;
pub mod error;
pub mod message;

pub use auth::AuthMethod;
pub use codec::WllCodec;
pub use endpoint::{endpoints, HealthResponse};
pub use error::{ProtocolError, ProtocolResult};
pub use message::{
    RefUpdateMsg, RefUpdateResultMsg, WllMessage, PROTOCOL_VERSION, MAX_MESSAGE_SIZE,
    capabilities,
};
