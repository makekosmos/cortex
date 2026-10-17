pub use ark_application::sync_storage::*;

pub(crate) const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
pub(crate) const KNOWN_PEERS_KEY: &str = "sync.peers";
pub(crate) const REMOVED_PEERS_KEY: &str = "sync.removed_peers";
pub(crate) const PEER_TRANSPORT_KEYS_KEY: &str = "sync.peer_transport_keys";
