use std::collections::{HashMap, HashSet};

use crate::hlc::HLC;
use crate::types::{PeerRecord, SyncEntity, VersionVector};

use super::{MAX_BATCH_BYTES, MAX_BATCH_SIZE};

pub fn is_usage_entity(entity: &SyncEntity) -> bool {
    matches!(
        entity.entity_type.as_str(),
        "usage_session" | "usage_event" | "usage_day"
    )
}

pub fn should_send_entity(remote: &VersionVector, entity: &SyncEntity) -> bool {
    if let (Some(device_id), Some(seq)) = (&entity.origin_device_id, entity.origin_seq) {
        return remote
            .get(&format!("@usage:{device_id}"))
            .and_then(|value| value.parse::<u64>().ok())
            .is_none_or(|cursor| seq > cursor);
    }
    remote
        .get(&entity.id)
        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc))
}

pub fn observe_non_usage_entity(vector: &mut VersionVector, entity: &SyncEntity) -> bool {
    if is_usage_entity(entity) {
        return false;
    }
    let changed = vector.get(&entity.id) != Some(&entity.hlc);
    vector.insert(entity.id.clone(), entity.hlc.clone());
    changed
}

pub fn merge_usage_cursors(target: &mut VersionVector, source: &VersionVector) {
    for (key, value) in source {
        if key.starts_with("@usage:") {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// Compute which entity IDs from `remote` are missing or outdated in `local`.
pub fn compute_vector_diff(local: &VersionVector, remote: &VersionVector) -> HashSet<String> {
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
pub fn compute_local_excess(local: &VersionVector, remote: &VersionVector) -> HashSet<String> {
    compute_vector_diff(remote, local)
}

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

/// Merge incoming peer records into existing ones.
pub fn merge_peer_records(existing: &[PeerRecord], incoming: &[PeerRecord]) -> Vec<PeerRecord> {
    let mut map = HashMap::new();

    for peer in existing {
        map.insert(peer.device_id.clone(), peer.clone());
    }

    for inc in incoming {
        match map.get_mut(&inc.device_id) {
            None => {
                map.insert(inc.device_id.clone(), inc.clone());
            }
            Some(current) => {
                let mut addr_set: HashSet<String> = current.addresses.iter().cloned().collect();
                for addr in &inc.addresses {
                    addr_set.insert(addr.clone());
                }
                current.addresses = addr_set.into_iter().collect();

                if inc.last_seen > current.last_seen {
                    current.last_seen.clone_from(&inc.last_seen);
                    current.device_name.clone_from(&inc.device_name);
                    if inc.last_address.is_some() {
                        current.last_address.clone_from(&inc.last_address);
                    }
                }
            }
        }
    }

    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compute_vector_diff_missing() {
        let local = VersionVector::new();
        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn compute_vector_diff_outdated() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn compute_vector_diff_up_to_date() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.is_empty());
    }

    #[test]
    fn compute_local_excess_finds_local_only_entities() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );
        local.insert(
            "e2".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let excess = compute_local_excess(&local, &remote);
        assert!(excess.contains("e2"));
        assert!(!excess.contains("e1"));
    }

    #[test]
    fn split_into_batches_by_count() {
        let entities: Vec<SyncEntity> = (0..250)
            .map(|i| SyncEntity {
                entity_type: "todo".to_string(),
                id: format!("t{i}"),
                data: serde_json::Map::new(),
                hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            })
            .collect();

        let batches = split_into_batches(&entities);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 100);
        assert_eq!(batches[1].len(), 100);
        assert_eq!(batches[2].len(), 50);
    }

    #[test]
    fn split_into_batches_by_bytes() {
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
                    origin_device_id: None,
                    origin_seq: None,
                }
            })
            .collect();

        let batches = split_into_batches(&entities);
        assert!(batches.len() >= 2);
        for batch in &batches {
            let total: usize = batch
                .iter()
                .map(|entity| serde_json::to_string(entity).unwrap().len())
                .sum();
            if batch.len() > 1 {
                assert!(total <= MAX_BATCH_BYTES + 300_000);
            }
        }
    }

    #[test]
    fn split_into_batches_empty() {
        let batches = split_into_batches(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn merge_peer_records_adds_new_peer() {
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
    fn merge_peer_records_unions_addresses_and_keeps_newest_metadata() {
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
    fn merge_peer_records_keeps_older_last_address() {
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
        assert_eq!(merged[0].device_name, "Phone");
        assert_eq!(merged[0].last_address, Some("10.0.0.1:21531".to_string()));
    }
}
