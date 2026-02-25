use async_trait::async_trait;
use wll_ledger::Receipt;
use wll_types::{ObjectId, WorldlineId};

use crate::error::SyncResult;
use crate::types::{RefRejection, RefUpdate};

/// Transport interface for remote WLL repositories.
#[async_trait]
pub trait RemoteTransport: Send + Sync {
    async fn list_refs(&self) -> SyncResult<Vec<(String, [u8; 32])>>;
    async fn fetch_objects(&self, wants: &[ObjectId], haves: &[ObjectId]) -> SyncResult<Vec<u8>>;
    async fn fetch_receipts(&self, worldlines: &[WorldlineId], since: Option<u64>) -> SyncResult<Vec<Receipt>>;
    async fn push_pack(&self, pack_bytes: &[u8]) -> SyncResult<()>;
    async fn push_receipts(&self, receipts: &[Receipt]) -> SyncResult<()>;
    async fn update_refs(&self, updates: &[RefUpdate]) -> SyncResult<Vec<RefRejection>>;
}
