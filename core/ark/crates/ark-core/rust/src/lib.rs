// Test code freely uses unwrap() — invariants are asserted by the surrounding
// test harness. Production paths use unwrap_or_else(|e| e.into_inner()) for
// Mutex poison recovery (см. forbidden.md → Mutex discipline) and expect()
// for genuine invariants.
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod beacon;
pub mod canonical_types;
pub mod data_platform;
pub mod db;
pub mod delphi;
pub mod events;
pub mod ffi;
pub mod hlc;
pub mod host;
#[cfg(feature = "iroh-spike")]
pub mod iroh_transport;
pub mod mesh;
pub mod net;
pub mod pomodoro;
pub mod protocol;
pub mod relay_sync;
pub mod relay_transport;
pub mod schema;
pub mod space;
pub mod sync_client;
pub mod sync_server;
pub mod sync_transport;
pub mod transport_select;
pub mod type_registry;
pub mod types;

uniffi::setup_scaffolding!();

// Re-export key types
pub use beacon::{
    BeaconPayload, BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions, BEACON_PORT,
    BEACON_TYPE, PEER_TTL_MS,
};
pub use db::SqliteStorageBackend;
pub use hlc::HLC;
pub use host::{get_host_device_name, get_own_addresses};
pub use protocol::{
    compute_local_excess, compute_vector_diff, deserialize_message, generate_id,
    merge_peer_records, serialize_message, split_into_batches, LanSyncMessage, LAN_SYNC_PORT,
    MAX_BATCH_BYTES, MAX_BATCH_SIZE, PROTOCOL_VERSION,
};
pub use sync_server::StorageBackend;
pub use types::{
    Area, ArkObject, ArkObjectSummary, ArkObjectWrite, Heading, LoadAllData, ObjectLink,
    ObjectType, PeerRecord, Project, SyncEntity, Tag, TodoItem, TrackedApp, UsageEvent,
    UsageSession, VersionVector,
};
