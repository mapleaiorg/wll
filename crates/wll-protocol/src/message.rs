use serde::{Deserialize, Serialize};
use wll_types::{ObjectId, WorldlineId};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// All message types in the WLL protocol.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WllMessage {
    Hello { version: u32, capabilities: Vec<String> },
    HelloAck { version: u32, capabilities: Vec<String> },
    ListRefsRequest { prefix: Option<String> },
    ListRefsResponse { refs: Vec<(String, [u8; 32])> },
    WantRequest { wants: Vec<ObjectId>, haves: Vec<ObjectId>, depth: Option<u32> },
    AckResponse { common: Vec<ObjectId> },
    PackData { pack_bytes: Vec<u8> },
    PackAck { checksum: [u8; 32], object_count: u32 },
    ReceiptBatch { worldline: WorldlineId, receipts_data: Vec<u8>, count: u32 },
    ReceiptAck { worldline: WorldlineId, through_seq: u64 },
    RefUpdateRequest { updates: Vec<RefUpdateMsg> },
    RefUpdateResponse { results: Vec<RefUpdateResultMsg> },
    Error { code: u32, message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefUpdateMsg {
    pub name: String,
    pub old_hash: Option<[u8; 32]>,
    pub new_hash: [u8; 32],
    pub force: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RefUpdateResultMsg {
    Ok { name: String },
    Rejected { name: String, reason: String },
}

impl WllMessage {
    pub fn type_tag(&self) -> u8 {
        match self {
            Self::Hello { .. } => 1,
            Self::HelloAck { .. } => 2,
            Self::ListRefsRequest { .. } => 3,
            Self::ListRefsResponse { .. } => 4,
            Self::WantRequest { .. } => 5,
            Self::AckResponse { .. } => 6,
            Self::PackData { .. } => 7,
            Self::PackAck { .. } => 8,
            Self::ReceiptBatch { .. } => 9,
            Self::ReceiptAck { .. } => 10,
            Self::RefUpdateRequest { .. } => 11,
            Self::RefUpdateResponse { .. } => 12,
            Self::Error { .. } => 255,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "Hello",
            Self::HelloAck { .. } => "HelloAck",
            Self::ListRefsRequest { .. } => "ListRefsRequest",
            Self::ListRefsResponse { .. } => "ListRefsResponse",
            Self::WantRequest { .. } => "WantRequest",
            Self::AckResponse { .. } => "AckResponse",
            Self::PackData { .. } => "PackData",
            Self::PackAck { .. } => "PackAck",
            Self::ReceiptBatch { .. } => "ReceiptBatch",
            Self::ReceiptAck { .. } => "ReceiptAck",
            Self::RefUpdateRequest { .. } => "RefUpdateRequest",
            Self::RefUpdateResponse { .. } => "RefUpdateResponse",
            Self::Error { .. } => "Error",
        }
    }
}

pub mod capabilities {
    pub const PACK_V1: &str = "pack-v1";
    pub const RECEIPT_CHAIN: &str = "receipt-chain";
    pub const DELTA_COMPRESSION: &str = "delta-compression";
    pub const SHALLOW_CLONE: &str = "shallow-clone";
}
