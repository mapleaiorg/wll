use crate::error::{ProtocolError, ProtocolResult};
use crate::message::{WllMessage, MAX_MESSAGE_SIZE};

/// Codec for encoding/decoding WLL protocol messages.
pub struct WllCodec;

impl WllCodec {
    /// Encode a message with framing: [4 bytes len][1 byte tag][payload]
    pub fn encode(msg: &WllMessage) -> ProtocolResult<Vec<u8>> {
        let payload = bincode::serialize(msg)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?;
        if payload.len() > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: payload.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }
        let len = (payload.len() + 1) as u32;
        let mut buf = Vec::with_capacity(4 + 1 + payload.len());
        buf.extend_from_slice(&len.to_be_bytes());
        buf.push(msg.type_tag());
        buf.extend_from_slice(&payload);
        Ok(buf)
    }

    /// Decode a framed message. Returns (message, bytes_consumed).
    pub fn decode(data: &[u8]) -> ProtocolResult<(WllMessage, usize)> {
        if data.len() < 5 {
            return Err(ProtocolError::FramingError("too short".into()));
        }
        let len = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
        if len < 1 {
            return Err(ProtocolError::FramingError("zero-length frame".into()));
        }
        if len - 1 > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::MessageTooLarge { size: len - 1, max: MAX_MESSAGE_SIZE });
        }
        let total = 4 + len;
        if data.len() < total {
            return Err(ProtocolError::FramingError(format!(
                "incomplete: have {}, need {}", data.len(), total
            )));
        }
        let payload = &data[5..total];
        let msg: WllMessage = bincode::deserialize(payload)
            .map_err(|e| ProtocolError::Deserialization(e.to_string()))?;
        Ok((msg, total))
    }

    /// Encode payload only (no framing).
    pub fn encode_payload(msg: &WllMessage) -> ProtocolResult<Vec<u8>> {
        bincode::serialize(msg).map_err(|e| ProtocolError::Serialization(e.to_string()))
    }

    /// Decode payload only (no framing).
    pub fn decode_payload(data: &[u8]) -> ProtocolResult<WllMessage> {
        bincode::deserialize(data).map_err(|e| ProtocolError::Deserialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::*;
    use wll_types::{ObjectId, WorldlineId};
    use wll_types::identity::IdentityMaterial;

    fn wl() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([1u8; 32]))
    }

    macro_rules! roundtrip_test {
        ($name:ident, $msg:expr) => {
            #[test]
            fn $name() {
                let msg = $msg;
                let encoded = WllCodec::encode(&msg).unwrap();
                let (decoded, consumed) = WllCodec::decode(&encoded).unwrap();
                assert_eq!(consumed, encoded.len());
                assert_eq!(decoded.type_tag(), msg.type_tag());
            }
        };
    }

    roundtrip_test!(hello_roundtrip, WllMessage::Hello {
        version: PROTOCOL_VERSION,
        capabilities: vec!["pack-v1".into()],
    });

    roundtrip_test!(hello_ack_roundtrip, WllMessage::HelloAck {
        version: PROTOCOL_VERSION,
        capabilities: vec![],
    });

    roundtrip_test!(list_refs_request_roundtrip, WllMessage::ListRefsRequest {
        prefix: Some("refs/heads/".into()),
    });

    roundtrip_test!(list_refs_response_roundtrip, WllMessage::ListRefsResponse {
        refs: vec![("main".into(), [1u8; 32])],
    });

    roundtrip_test!(want_request_roundtrip, WllMessage::WantRequest {
        wants: vec![ObjectId::from_bytes(b"want")],
        haves: vec![ObjectId::from_bytes(b"have")],
        depth: Some(10),
    });

    roundtrip_test!(ack_response_roundtrip, WllMessage::AckResponse {
        common: vec![ObjectId::null()],
    });

    roundtrip_test!(pack_data_roundtrip, WllMessage::PackData {
        pack_bytes: vec![1, 2, 3, 4, 5],
    });

    roundtrip_test!(pack_ack_roundtrip, WllMessage::PackAck {
        checksum: [0xAB; 32],
        object_count: 42,
    });

    roundtrip_test!(receipt_batch_roundtrip, WllMessage::ReceiptBatch {
        worldline: wl(),
        receipts_data: vec![10, 20, 30],
        count: 3,
    });

    roundtrip_test!(receipt_ack_roundtrip, WllMessage::ReceiptAck {
        worldline: wl(),
        through_seq: 100,
    });

    roundtrip_test!(ref_update_request_roundtrip, WllMessage::RefUpdateRequest {
        updates: vec![RefUpdateMsg {
            name: "main".into(),
            old_hash: Some([1; 32]),
            new_hash: [2; 32],
            force: false,
        }],
    });

    roundtrip_test!(ref_update_response_roundtrip, WllMessage::RefUpdateResponse {
        results: vec![
            RefUpdateResultMsg::Ok { name: "main".into() },
            RefUpdateResultMsg::Rejected { name: "dev".into(), reason: "non-ff".into() },
        ],
    });

    roundtrip_test!(error_roundtrip, WllMessage::Error {
        code: 404,
        message: "not found".into(),
    });

    #[test]
    fn type_tags_unique() {
        let msgs: Vec<WllMessage> = vec![
            WllMessage::Hello { version: 1, capabilities: vec![] },
            WllMessage::HelloAck { version: 1, capabilities: vec![] },
            WllMessage::ListRefsRequest { prefix: None },
            WllMessage::ListRefsResponse { refs: vec![] },
            WllMessage::WantRequest { wants: vec![], haves: vec![], depth: None },
            WllMessage::AckResponse { common: vec![] },
            WllMessage::PackData { pack_bytes: vec![] },
            WllMessage::PackAck { checksum: [0; 32], object_count: 0 },
            WllMessage::ReceiptBatch { worldline: wl(), receipts_data: vec![], count: 0 },
            WllMessage::ReceiptAck { worldline: wl(), through_seq: 0 },
            WllMessage::RefUpdateRequest { updates: vec![] },
            WllMessage::RefUpdateResponse { results: vec![] },
            WllMessage::Error { code: 0, message: String::new() },
        ];
        let mut tags: Vec<u8> = msgs.iter().map(|m| m.type_tag()).collect();
        let len = tags.len();
        tags.sort();
        tags.dedup();
        assert_eq!(tags.len(), len, "type tags should be unique");
    }

    #[test]
    fn type_names_correct() {
        let msg = WllMessage::Hello { version: 1, capabilities: vec![] };
        assert_eq!(msg.type_name(), "Hello");
        let msg = WllMessage::Error { code: 0, message: String::new() };
        assert_eq!(msg.type_name(), "Error");
    }

    #[test]
    fn decode_truncated() {
        let err = WllCodec::decode(&[0, 0, 0]).unwrap_err();
        assert!(matches!(err, ProtocolError::FramingError(_)));
    }

    #[test]
    fn decode_zero_length() {
        let data = [0u8, 0, 0, 0, 0]; // length = 0
        let err = WllCodec::decode(&data).unwrap_err();
        assert!(matches!(err, ProtocolError::FramingError(_)));
    }

    #[test]
    fn payload_roundtrip() {
        let msg = WllMessage::Hello { version: 1, capabilities: vec!["test".into()] };
        let bytes = WllCodec::encode_payload(&msg).unwrap();
        let decoded = WllCodec::decode_payload(&bytes).unwrap();
        assert_eq!(decoded.type_tag(), msg.type_tag());
    }

    #[test]
    fn capabilities_constants() {
        assert_eq!(capabilities::PACK_V1, "pack-v1");
        assert_eq!(capabilities::RECEIPT_CHAIN, "receipt-chain");
    }
}
