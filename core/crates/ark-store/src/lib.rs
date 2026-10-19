pub mod committed_feed;
pub mod data_platform;
pub mod data_volume;
pub mod integration_replication;
pub mod object_types;
pub mod objects;
pub mod open;
pub mod operation_receipt;
pub mod schema;
pub mod snapshot;
pub mod sync_kv;
pub mod sync_versions;
pub mod task_merge;
pub mod type_registry;
pub mod usage_sync_log;
pub mod writer;

pub const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";

#[cfg(test)]
mod schema_tests;
