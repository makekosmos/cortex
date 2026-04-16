pub mod beacon;
pub mod db;
pub mod ffi;
pub mod hlc;
pub mod host;
pub mod mesh;
pub mod net;
pub mod protocol;
pub mod relay_transport;
pub mod schema;
pub mod space;
pub mod sync_client;
pub mod sync_server;
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
    Area, Heading, LoadAllData, PeerRecord, Project, SyncEntity, Tag, TodoItem, TrackedApp,
    UsageEvent, UsageSession, VersionVector,
};
