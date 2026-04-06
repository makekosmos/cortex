pub mod db;
pub mod hlc;
pub mod protocol;
pub mod schema;
pub mod space;
pub mod sync_client;
pub mod sync_server;
pub mod types;

uniffi::setup_scaffolding!();

// Re-export key types
pub use hlc::HLC;
pub use protocol::{
    compute_local_excess, compute_vector_diff, generate_id, merge_peer_records,
    serialize_message, deserialize_message, split_into_batches,
    LanSyncMessage, LAN_SYNC_PORT, PROTOCOL_VERSION, MAX_BATCH_SIZE, MAX_BATCH_BYTES,
};
pub use sync_server::StorageBackend;
pub use types::{
    Area, Heading, LoadAllData, PeerRecord, Project, SyncEntity, Tag, TodoItem, VersionVector,
};
