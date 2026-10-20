// Test code freely uses unwrap() — invariants are asserted by the surrounding
// test harness. Production paths use unwrap_or_else(|e| e.into_inner()) for
// Mutex poison recovery (см. forbidden.md → Mutex discipline) and expect()
// for genuine invariants.
#![cfg_attr(test, allow(clippy::unwrap_used))]

#[cfg(test)]
extern crate self as ark_core;

pub mod application;
pub mod canonical_types;
pub mod db;
pub mod delphi;
pub mod direct_client;
pub mod events;
pub mod hlc;
pub mod integration_replication;
pub mod iroh_transport;
pub mod mesh;
pub mod relay_sync;
pub mod replication_storage;
pub mod service;
pub mod space;
pub mod sync_server;
pub mod sync_transport;
mod task_set;
#[cfg(test)]
pub(crate) mod test_support;
pub use ark_sync::transport_select;
pub mod type_registry;
pub mod types;

pub use ark_net::{beacon, host, net, relay as relay_transport, sync_bind};

// Re-export key types
pub use beacon::{
    BeaconPayload, BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions, BEACON_PORT,
    BEACON_TYPE, PEER_TTL_MS,
};
pub use db::SqliteStorageBackend;
pub use hlc::HLC;
pub use host::{get_host_device_name, get_own_addresses};
pub use replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
    SyncRuntimeStorage,
};
pub use sync_bind::SyncBind;
pub use types::{
    Area, ArkObject, ArkObjectSummary, ArkObjectWrite, Heading, LoadAllData, ObjectLink,
    ObjectType, PeerRecord, Project, SyncEntity, Tag, TodoItem, TrackedApp, UsageEvent,
    UsageSession, VersionVector,
};
