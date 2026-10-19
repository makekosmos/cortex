pub mod committed_feed;
pub mod data_platform;
pub mod data_volume;
pub mod device_registry;
pub mod integration_replication;
pub mod legacy_entities;
pub mod object_link_mutation;
pub mod object_types;
pub mod object_write_receipts;
pub mod objects;
pub mod open;
pub mod operation_receipt;
pub mod pending_objects;
pub mod schema;
pub mod snapshot;
pub mod sync_kv;
pub mod sync_tombstones;
pub mod sync_versions;
pub mod task_merge;
pub mod type_registry;
pub mod usage;
pub mod usage_history;
pub mod usage_mutation;
pub mod usage_queries;
pub mod usage_sync_log;
pub mod writer;

pub const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";

#[cfg(test)]
mod schema_tests;
