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

/// Apply the sender's `usage_complete_through` claims after its final sync
/// page: raise each `@usage:` cursor to the coverage the sender proved it
/// can serve (compacted holes count as covered on the sender's side).
/// Never lowers a cursor — the claim is a floor, not a reposition.
pub fn apply_usage_complete_through(
    vector: &mut VersionVector,
    complete_through: &HashMap<String, u64>,
) -> bool {
    let mut changed = false;
    for (device_id, seq) in complete_through {
        let key = format!("@usage:{device_id}");
        let current = vector
            .get(&key)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        if *seq > current {
            vector.insert(key, seq.to_string());
            changed = true;
        }
    }
    changed
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
