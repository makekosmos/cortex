use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

use crate::hlc::HLC;
use crate::types::*;

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
    serde_json::to_string(msg).unwrap_or_default()
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
// Hello HMAC authentication
// ---------------------------------------------------------------------------

pub fn normalize_auth_secret(secret: Option<String>) -> Option<String> {
    secret
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn generate_auth_nonce() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

pub fn compute_hello_auth_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
) -> String {
    let message = format!("ark-sync-v1:hello:{space_id}:{device_id}:{nonce}");
    hmac_sha256_hex(secret.as_bytes(), message.as_bytes())
}

pub fn verify_hello_auth_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
    provided_hmac: &str,
) -> bool {
    let expected = compute_hello_auth_hmac(secret, space_id, device_id, nonce);
    constant_time_eq(expected.as_bytes(), provided_hmac.as_bytes())
}

fn hmac_sha256_hex(secret: &[u8], message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;

    let mut key = [0u8; BLOCK_SIZE];
    if secret.len() > BLOCK_SIZE {
        let digest = Sha256::digest(secret);
        key[..digest.len()].copy_from_slice(&digest);
    } else {
        key[..secret.len()].copy_from_slice(secret);
    }

    let mut outer_key_pad = [0x5c_u8; BLOCK_SIZE];
    let mut inner_key_pad = [0x36_u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        outer_key_pad[i] ^= key[i];
        inner_key_pad[i] ^= key[i];
    }

    let mut inner = Sha256::new();
    inner.update(inner_key_pad);
    inner.update(message);
    let inner_result = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_key_pad);
    outer.update(inner_result);
    hex_encode(&outer.finalize())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let max_len = a.len().max(b.len());
    for i in 0..max_len {
        let left = a.get(i).copied().unwrap_or(0);
        let right = b.get(i).copied().unwrap_or(0);
        diff |= (left ^ right) as usize;
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
// Version vector diff
// ---------------------------------------------------------------------------

/// Compute which entity IDs from `remote` are missing or outdated in `local`.
pub fn compute_vector_diff(
    local: &crate::types::VersionVector,
    remote: &crate::types::VersionVector,
) -> HashSet<String> {
    let mut needed = HashSet::new();
    for (entity_id, remote_hlc) in remote {
        match local.get(entity_id) {
            None => {
                needed.insert(entity_id.clone());
            }
            Some(local_hlc) => {
                if HLC::is_newer(remote_hlc, local_hlc) {
                    needed.insert(entity_id.clone());
                }
            }
        }
    }
    needed
}

/// Compute which entity IDs the local side has that the remote doesn't (or has older).
pub fn compute_local_excess(
    local: &crate::types::VersionVector,
    remote: &crate::types::VersionVector,
) -> HashSet<String> {
    compute_vector_diff(remote, local)
}

// ---------------------------------------------------------------------------
// Batch splitting
// ---------------------------------------------------------------------------

/// Split entities into batches respecting MAX_BATCH_SIZE and MAX_BATCH_BYTES.
pub fn split_into_batches(entities: &[SyncEntity]) -> Vec<Vec<SyncEntity>> {
    if entities.is_empty() {
        return vec![];
    }

    let mut batches: Vec<Vec<SyncEntity>> = Vec::new();
    let mut current: Vec<SyncEntity> = Vec::new();
    let mut current_bytes: usize = 0;

    for entity in entities {
        let entity_bytes = serde_json::to_string(entity).unwrap_or_default().len();

        if !current.is_empty()
            && (current.len() >= MAX_BATCH_SIZE || current_bytes + entity_bytes > MAX_BATCH_BYTES)
        {
            batches.push(current);
            current = Vec::new();
            current_bytes = 0;
        }

        current.push(entity.clone());
        current_bytes += entity_bytes;
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

// ---------------------------------------------------------------------------
// Peer record merging
// ---------------------------------------------------------------------------

/// Merge incoming peer records into existing ones.
pub fn merge_peer_records(existing: &[PeerRecord], incoming: &[PeerRecord]) -> Vec<PeerRecord> {
    let mut map = std::collections::HashMap::new();

    for peer in existing {
        map.insert(peer.device_id.clone(), peer.clone());
    }

    for inc in incoming {
        match map.get_mut(&inc.device_id) {
            None => {
                map.insert(inc.device_id.clone(), inc.clone());
            }
            Some(current) => {
                // Union addresses
                let mut addr_set: HashSet<String> = current.addresses.iter().cloned().collect();
                for addr in &inc.addresses {
                    addr_set.insert(addr.clone());
                }
                current.addresses = addr_set.into_iter().collect();

                // Keep newest last_seen
                if inc.last_seen > current.last_seen {
                    current.last_seen = inc.last_seen.clone();
                    current.device_name = inc.device_name.clone();
                    if inc.last_address.is_some() {
                        current.last_address = inc.last_address.clone();
                    }
                }
            }
        }
    }

    map.into_values().collect()
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
    fn test_compute_vector_diff_missing() {
        let local = crate::types::VersionVector::new();
        let mut remote = crate::types::VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn test_compute_vector_diff_outdated() {
        let mut local = crate::types::VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = crate::types::VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn test_compute_vector_diff_up_to_date() {
        let mut local = crate::types::VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = crate::types::VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.is_empty());
    }

    #[test]
    fn test_compute_local_excess() {
        let mut local = crate::types::VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );
        local.insert(
            "e2".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = crate::types::VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let excess = compute_local_excess(&local, &remote);
        assert!(excess.contains("e2"));
        assert!(!excess.contains("e1"));
    }

    #[test]
    fn test_split_into_batches_count() {
        let entities: Vec<SyncEntity> = (0..250)
            .map(|i| SyncEntity {
                entity_type: "todo".to_string(),
                id: format!("t{i}"),
                data: serde_json::Map::new(),
                hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
                deleted: None,
            })
            .collect();

        let batches = split_into_batches(&entities);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 100);
        assert_eq!(batches[1].len(), 100);
        assert_eq!(batches[2].len(), 50);
    }

    #[test]
    fn test_split_into_batches_bytes() {
        // Create entities with large data to trigger byte limit
        let entities: Vec<SyncEntity> = (0..10)
            .map(|i| {
                let mut data = serde_json::Map::new();
                data.insert("big".to_string(), json!("x".repeat(200_000)));
                SyncEntity {
                    entity_type: "todo".to_string(),
                    id: format!("t{i}"),
                    data,
                    hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
                    deleted: None,
                }
            })
            .collect();

        let batches = split_into_batches(&entities);
        // Each entity is ~200KB, so ~5 per 1MB batch
        assert!(batches.len() >= 2);
        for batch in &batches {
            let total: usize = batch
                .iter()
                .map(|e| serde_json::to_string(e).unwrap().len())
                .sum();
            // First entity in a batch is always allowed even if over limit
            if batch.len() > 1 {
                assert!(total <= MAX_BATCH_BYTES + 300_000); // some slack for the first-entity rule
            }
        }
    }

    #[test]
    fn test_split_into_batches_empty() {
        let batches = split_into_batches(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_merge_peer_records_new() {
        let existing: Vec<PeerRecord> = vec![];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: None,
        }];
        let merged = merge_peer_records(&existing, &incoming);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].device_id, "d1");
    }

    #[test]
    fn test_merge_peer_records_union_addresses() {
        let existing = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: None,
        }];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone v2".to_string(),
            addresses: vec![
                "10.0.0.1:21531".to_string(),
                "192.168.1.50:21531".to_string(),
            ],
            last_seen: "2026-01-02T00:00:00.000Z".to_string(),
            last_address: Some("192.168.1.50:21531".to_string()),
        }];

        let merged = merge_peer_records(&existing, &incoming);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].addresses.contains(&"10.0.0.1:21531".to_string()));
        assert!(merged[0]
            .addresses
            .contains(&"192.168.1.50:21531".to_string()));
        assert_eq!(merged[0].device_name, "Phone v2");
        assert_eq!(merged[0].last_seen, "2026-01-02T00:00:00.000Z");
        assert_eq!(
            merged[0].last_address,
            Some("192.168.1.50:21531".to_string())
        );
    }

    #[test]
    fn test_merge_peer_records_keeps_older_last_address() {
        let existing = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-02T00:00:00.000Z".to_string(),
            last_address: Some("10.0.0.1:21531".to_string()),
        }];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone old".to_string(),
            addresses: vec!["10.0.0.2:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: Some("10.0.0.2:21531".to_string()),
        }];

        let merged = merge_peer_records(&existing, &incoming);
        // Existing is newer, so device_name and last_address should stay
        assert_eq!(merged[0].device_name, "Phone");
        assert_eq!(merged[0].last_address, Some("10.0.0.1:21531".to_string()));
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
            _ => panic!("Expected Hello"),
        }
    }

    #[test]
    fn test_hello_hmac_helpers_are_deterministic_and_verify() {
        let nonce = "0123456789abcdef";
        let hmac = compute_hello_auth_hmac("secret", "space-a", "device-a", nonce);

        assert_eq!(hmac.len(), 64);
        assert!(verify_hello_auth_hmac(
            "secret", "space-a", "device-a", nonce, &hmac
        ));
        assert!(!verify_hello_auth_hmac(
            "wrong", "space-a", "device-a", nonce, &hmac
        ));
        assert!(!verify_hello_auth_hmac(
            "secret", "space-a", "device-b", nonce, &hmac
        ));
    }

    #[test]
    fn test_auth_nonce_is_random_hex_64() {
        let nonce_a = generate_auth_nonce();
        let nonce_b = generate_auth_nonce();

        assert_eq!(nonce_a.len(), 64);
        assert!(nonce_a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(nonce_a, nonce_b);
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
