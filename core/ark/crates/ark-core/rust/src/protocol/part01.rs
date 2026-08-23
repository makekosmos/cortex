use serde::{Deserialize, Serialize};

use crate::types::*;

#[path = "../protocol/auth.rs"] mod auth;
#[path = "../protocol/vector.rs"] mod vector;

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

