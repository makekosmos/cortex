//! Worker-facing protocol types shared between the Mundus Engine broker and
//! standalone package workers (e.g. `ark-markdown-bridge` in the integrations
//! repository). This crate is the stable, pinned surface for out-of-tree
//! workers; `mundus-engine::package_worker_protocol` re-exports it.

use serde::{Deserialize, Serialize};

/// Bootstrap `bridge_config` payload for bridge workers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeWorkerConfig {
    pub vault_root: String,
    pub state_root: String,
    pub selected_types: Vec<String>,
    pub editable_fields: Vec<String>,
    pub readonly_fields: Vec<String>,
}

/// Bridge sync status reported in `worker.heartbeat` messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeStatus {
    pub last_sync: Option<String>,
    pub conflict_count: u32,
    pub last_conflict_at: Option<String>,
}
