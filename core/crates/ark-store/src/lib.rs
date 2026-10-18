pub mod committed_feed;
pub mod data_platform;
pub mod data_volume;
pub mod integration_replication;
pub mod open;
pub mod schema;
pub mod sync_kv;
pub mod type_registry;
pub mod usage_sync_log;

pub const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";

#[cfg(test)]
mod schema_tests;
