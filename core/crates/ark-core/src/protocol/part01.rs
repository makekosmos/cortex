use serde::{Deserialize, Serialize};

use crate::types::*;

#[path = "../protocol/auth.rs"]
mod auth;
#[path = "../protocol/vector.rs"]
mod vector;

pub use auth::{
    compute_hello_auth_hmac, generate_auth_nonce, normalize_auth_secret, verify_hello_auth_hmac,
};
pub use vector::{
    apply_usage_complete_through, compute_local_excess, compute_vector_diff, is_usage_entity,
    merge_peer_records, merge_usage_cursors, merge_vector_entries, observe_non_usage_entity,
    should_send_entity, split_into_batches,
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
        /// Sender's OS (`std::env::consts::OS` spelling: "macos", "windows",
        /// ...) — shown in the device list. Optional so peers built before
        /// this field keep parsing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        platform: Option<String>,
        /// Sender's product version label (the Engine build, not the crate
        /// version). Optional for the same compat reason as `platform`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app_version: Option<String>,
        /// Bounded wire versions understood by this peer. Missing means a
        /// legacy peer that supports only `protocol_version`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supported_protocol_versions: Option<Vec<u32>>,
        /// Stable capability IDs understood by this peer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supported_capabilities: Option<Vec<String>>,
        /// Capabilities selected by the responder. Initiators omit this.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        negotiated_capabilities: Option<Vec<String>>,
        /// Domain-separated HMAC over this offer or the full selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        negotiation_hmac: Option<String>,
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
        // Per usage origin device, the highest seq the sender can serve
        // completely — its own contiguous `@usage:` coverage (compaction
        // only deletes refs below it). Sent only on the final `is_last`
        // batch: the receiver raises its `@usage:` cursors past compacted
        // holes once the whole stream is applied. Optional so older peers
        // (which ignore unknown fields) and older senders (field absent)
        // keep today's contiguous-only cursor advancement. KOS-302.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage_complete_through: Option<std::collections::HashMap<String, u64>>,
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

    /// The durable inbox has committed the message. This does not mean that
    /// the entity has been applied. Sent only after receipt capability
    /// negotiation succeeds.
    #[serde(rename = "live_receipt_ack")]
    LiveReceiptAck { change_id: String },

    /// Refuses an incompatible protocol offer before the peer is admitted.
    /// Relay transports broadcast this frame and use both IDs for routing.
    #[serde(rename = "protocol_rejected")]
    ProtocolRejected {
        origin_device_id: String,
        target_device_id: String,
        reason: String,
        supported_protocol_versions: Vec<u32>,
        offered_protocol_versions: Vec<u32>,
    },

    #[serde(rename = "signed_integration_frame")]
    SignedIntegrationFrame {
        frame: crate::integration_replication::SignedSyncEnvelope,
    },

    #[serde(rename = "signed_integration_ack")]
    SignedIntegrationAck { message_id: String, accepted: bool },

    /// KOS-369: the responder declined the consent prompt. Protocol-level
    /// rejection so the initiator shows «Подключение отклонено» instead of
    /// hanging; peers built before this variant can't parse it and simply
    /// get their connection closed.
    #[serde(rename = "pairing_rejected")]
    PairingRejected { device_id: String },

    /// Sent by the device that received a pairing request only after the
    /// user accepts it. The sender is still identified by the transport key;
    /// this field must match the Hello parked for that endpoint.
    #[serde(rename = "pairing_accepted")]
    PairingAccepted { device_id: String },

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
        LanSyncMessage::Hello { device_id, .. }
        | LanSyncMessage::PairingRejected { device_id }
        | LanSyncMessage::PairingAccepted { device_id } => Some(device_id.clone()),
        LanSyncMessage::ProtocolRejected {
            origin_device_id, ..
        } => Some(origin_device_id.clone()),
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
