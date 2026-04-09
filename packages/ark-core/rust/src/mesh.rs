//! Mesh coordinator — manages both LAN (SyncServer + SyncClient + Beacon) and
//! optional Relay transport simultaneously, deduplicating incoming changes so
//! a change arriving on both transports is processed only once.
//!
//! This module is not exposed via UniFFI. It is used internally by
//! `ArkCore::start_sync` when `relay_url` is `Some(...)`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::protocol::LanSyncMessage;

// ---------------------------------------------------------------------------
// MeshConfig — companion to FfiSyncConfig for internal use
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MeshConfig {
    pub lan_enabled: bool,
    pub relay_url: Option<String>,
    pub relay_api_key: Option<String>,
}

impl Default for MeshConfig {
    fn default() -> Self {
        MeshConfig {
            lan_enabled: true,
            relay_url: None,
            relay_api_key: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Dedup key
// ---------------------------------------------------------------------------

/// A (device_id, entity_id, hlc) triple used to deduplicate changes that
/// arrive on both the LAN and relay transports.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DedupKey {
    device_id: String,
    entity_id: String,
    hlc: String,
}

// ---------------------------------------------------------------------------
// MeshCoordinator
// ---------------------------------------------------------------------------

/// Coordinator that holds LAN and relay state and deduplicates incoming changes.
pub struct MeshCoordinator {
    config: MeshConfig,
    /// Seen (device_id, entity_id, hlc) triples — bounded by TTL/LRU in a
    /// production impl; here we keep a simple unbounded set (fine for Delphi
    /// session lifetimes).
    seen: Mutex<HashSet<DedupKey>>,
}

impl MeshCoordinator {
    pub fn new(config: MeshConfig) -> Arc<Self> {
        Arc::new(MeshCoordinator {
            config,
            seen: Mutex::new(HashSet::new()),
        })
    }

    /// Returns `true` if this change has NOT been seen before and should be
    /// processed; returns `false` if it is a duplicate.
    pub fn should_process(
        &self,
        device_id: &str,
        entity_id: &str,
        hlc: &str,
    ) -> bool {
        let key = DedupKey {
            device_id: device_id.to_string(),
            entity_id: entity_id.to_string(),
            hlc: hlc.to_string(),
        };
        let mut seen = self.seen.lock().unwrap();
        seen.insert(key) // returns true when newly inserted
    }

    /// Check whether relay is configured.
    pub fn has_relay(&self) -> bool {
        self.config.relay_url.is_some()
    }

    pub fn relay_url(&self) -> Option<&str> {
        self.config.relay_url.as_deref()
    }

    pub fn relay_api_key(&self) -> Option<&str> {
        self.config.relay_api_key.as_deref()
    }

    /// Extract (device_id, entity_id, hlc) from a LanSyncMessage for dedup.
    /// Returns None for message types that don't carry entity changes.
    ///
    /// The device_id is extracted from the HLC string, which has the format
    /// `<ISO8601>:<counter:06d>:<device_id>`.
    pub fn dedup_key_from_message(msg: &LanSyncMessage) -> Option<(String, String, String)> {
        match msg {
            LanSyncMessage::LiveChange { entity, .. } => {
                // Extract device_id from HLC: last segment after the second colon.
                let device_id = entity
                    .hlc
                    .splitn(3, ':')
                    .nth(2)
                    .unwrap_or("")
                    .to_string();
                Some((device_id, entity.id.clone(), entity.hlc.clone()))
            }
            LanSyncMessage::SyncChanges { entities, .. } => {
                // For batch syncs we don't dedup here (handled per-entity in the server).
                // Return None to let all batches through.
                let _ = entities;
                None
            }
            _ => None,
        }
    }
}
