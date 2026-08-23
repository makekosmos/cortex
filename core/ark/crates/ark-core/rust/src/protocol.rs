use serde::{Deserialize, Serialize};

use crate::types::*;

mod auth;
mod vector;

pub use auth::{
    compute_hello_auth_hmac, generate_auth_nonce, normalize_auth_secret, verify_hello_auth_hmac,
};
pub use vector::{
    compute_local_excess, compute_vector_diff, is_usage_entity, merge_peer_records,
    merge_usage_cursors, observe_non_usage_entity, should_send_entity, split_into_batches,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const LAN_SYNC_PORT: u16 = 21531;
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_BATCH_SIZE: usize = 100;
pub const MAX_BATCH_BYTES: usize = 1_048_576; // 1 MB
pub const BATCH_ACK_TIMEOUT_MS: u64 = 10_000;
pub const LIVE_ACK_TIMEOUT_MS: u64 = 5_000;
pub const MAX_RETRIES: u32 = 3;
pub const PING_INTERVAL_MS: u64 = 15_000;

// ---------------------------------------------------------------------------
// Protocol messages (snake_case JSON)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum LanSyncMessage {
    #[serde(rename = "hello")]
    Hello {
        protocol_version: u32,
        device_id: String,
        device_name: String,
        space_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        addresses: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth_nonce: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth_hmac: Option<String>,
    },

    #[serde(rename = "version_vector")]
    VersionVector {
        vector: crate::types::VersionVector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_device_id: Option<String>,
    },

    #[serde(rename = "sync_changes")]
    SyncChanges {
        batch_id: String,
        entities: Vec<SyncEntity>,
        is_last: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_device_id: Option<String>,
    },

    #[serde(rename = "sync_ack")]
    SyncAck { batch_id: String, accepted: usize },

    #[serde(rename = "live_change")]
    LiveChange {
        change_id: String,
        entity: SyncEntity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_device_id: Option<String>,
    },

    #[serde(rename = "live_ack")]
    LiveAck { change_id: String },

    #[serde(rename = "peer_list")]
    PeerList { peers: Vec<PeerRecord> },

    #[serde(rename = "ping")]
    Ping { ts: u64 },

    #[serde(rename = "pong")]
    Pong { ts: u64 },
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

pub fn serialize_message(msg: &LanSyncMessage) -> String {
    match serde_json::to_string(msg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ark-core] serialize_message failed: {e}");
            String::new()
        }
    }
}

pub fn deserialize_message(raw: &str) -> Option<LanSyncMessage> {
    serde_json::from_str(raw).ok()
}

pub fn message_origin_device_id(msg: &LanSyncMessage) -> Option<String> {
    match msg {
        LanSyncMessage::Hello { device_id, .. } => Some(device_id.clone()),
        LanSyncMessage::VersionVector {
            origin_device_id, ..
        }
        | LanSyncMessage::SyncChanges {
            origin_device_id, ..
        }
        | LanSyncMessage::LiveChange {
            origin_device_id, ..
        } => origin_device_id.clone(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

pub fn generate_id() -> String {
    use rand::Rng;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut rng = rand::thread_rng();
    let random: String = (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("{ts}-{random}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_hello_serialization() {
        let msg = LanSyncMessage::Hello {
            protocol_version: 1,
            device_id: "dev-1".to_string(),
            device_name: "MacBook".to_string(),
            space_id: "abc123".to_string(),
            addresses: Some(vec!["192.168.1.70:21531".to_string()]),
            auth_nonce: None,
            auth_hmac: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "hello");
        assert_eq!(parsed["protocol_version"], 1);
        assert_eq!(parsed["device_id"], "dev-1");
        assert_eq!(parsed["device_name"], "MacBook");
        assert_eq!(parsed["space_id"], "abc123");
    }

    #[test]
    fn test_version_vector_serialization() {
        let mut vector = crate::types::VersionVector::new();
        vector.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let msg = LanSyncMessage::VersionVector {
            vector,
            origin_device_id: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "version_vector");
        assert!(parsed["vector"]["e1"].is_string());
    }

    #[test]
    fn test_sync_changes_serialization() {
        let entity = SyncEntity {
            entity_type: "todo".to_string(),
            id: "t1".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let msg = LanSyncMessage::SyncChanges {
            batch_id: "123-abc".to_string(),
            entities: vec![entity],
            is_last: true,
            origin_device_id: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "sync_changes");
        assert_eq!(parsed["batch_id"], "123-abc");
        assert_eq!(parsed["is_last"], true);
        assert_eq!(parsed["entities"][0]["type"], "todo");
        // deleted should be omitted when None
        assert!(parsed["entities"][0].get("deleted").is_none());
    }

    #[test]
    fn test_sync_entity_deleted_field() {
        let entity = SyncEntity {
            entity_type: "todo".to_string(),
            id: "t1".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        let json_str = serde_json::to_string(&entity).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["deleted"], true);
        assert_eq!(parsed["type"], "todo");
    }

    #[test]
    fn test_live_change_serialization() {
        let entity = SyncEntity {
            entity_type: "project".to_string(),
            id: "p1".to_string(),
            data: {
                let mut m = serde_json::Map::new();
                m.insert("title".to_string(), json!("My Project"));
                m
            },
            hlc: "2026-01-01T00:00:00.000Z:000001:dev".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let msg = LanSyncMessage::LiveChange {
            change_id: "ch-1".to_string(),
            entity,
            origin_device_id: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "live_change");
        assert_eq!(parsed["change_id"], "ch-1");
        assert_eq!(parsed["entity"]["type"], "project");
    }

    #[test]
    fn test_ping_pong_serialization() {
        let ping = LanSyncMessage::Ping { ts: 1234567890 };
        let json_str = serialize_message(&ping);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "ping");
        assert_eq!(parsed["ts"], 1234567890u64);

        let pong = LanSyncMessage::Pong { ts: 1234567890 };
        let json_str = serialize_message(&pong);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "pong");
    }

    #[test]
    fn test_peer_list_serialization() {
        let msg = LanSyncMessage::PeerList {
            peers: vec![PeerRecord {
                device_id: "d1".to_string(),
                device_name: "Phone".to_string(),
                addresses: vec!["10.0.0.1:21531".to_string()],
                last_seen: "2026-01-01T00:00:00.000Z".to_string(),
                last_address: Some("10.0.0.1:21531".to_string()),
            }],
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "peer_list");
        assert_eq!(parsed["peers"][0]["device_id"], "d1");
    }

    #[test]
    fn test_message_roundtrip() {
        let msg = LanSyncMessage::SyncAck {
            batch_id: "b1".to_string(),
            accepted: 42,
        };
        let json_str = serialize_message(&msg);
        let deserialized = deserialize_message(&json_str).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.contains('-'));
        let parts: Vec<&str> = id.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].parse::<u128>().is_ok());
        assert_eq!(parts[1].len(), 6);
    }

    #[test]
    fn test_deserialize_ts_compatible_hello() {
        // JSON exactly as the TS implementation would produce
        let json = r#"{"type":"hello","protocol_version":1,"device_id":"dev-123","device_name":"Electron","space_id":"abcdef0123456789","addresses":["192.168.1.70:21531"]}"#;
        let msg = deserialize_message(json).unwrap();
        match msg {
            LanSyncMessage::Hello {
                protocol_version,
                device_id,
                device_name,
                space_id,
                addresses,
                auth_nonce,
                auth_hmac,
            } => {
                assert_eq!(protocol_version, 1);
                assert_eq!(device_id, "dev-123");
                assert_eq!(device_name, "Electron");
                assert_eq!(space_id, "abcdef0123456789");
                assert_eq!(addresses, Some(vec!["192.168.1.70:21531".to_string()]));
                assert_eq!(auth_nonce, None);
                assert_eq!(auth_hmac, None);
            }
            _ => unreachable!("Expected Hello"),
        }
    }

    #[test]
    fn test_deserialize_ts_compatible_sync_entity() {
        let json = r#"{"type":"todo","id":"550e8400-e29b-41d4-a716-446655440000","data":{"title":"Buy milk","isCompleted":false,"tagIds":["tag1"]},"hlc":"2026-03-28T14:30:00.123Z:000042:delphi-web-abc123"}"#;
        let entity: SyncEntity = serde_json::from_str(json).unwrap();
        assert_eq!(entity.entity_type, "todo");
        assert_eq!(entity.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(entity.data["title"], "Buy milk");
        assert!(entity.deleted.is_none());
    }
}
