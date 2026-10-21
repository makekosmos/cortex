pub mod book_cover;
pub mod canonical_asset_sources;
pub mod canonical_game;
pub mod canonical_migrations;
pub mod committed_feed;
pub mod data_platform;
pub mod data_volume;
pub mod database;
pub mod device_registry;
pub mod integration_replication;
pub mod legacy_entities;
mod legacy_mutation;
pub mod load_all;
pub mod object_link_mutation;
pub mod object_mutation;
pub mod object_types;
pub mod object_write;
pub mod object_write_receipts;
pub mod objects;
pub mod open;
pub mod operation_receipt;
pub mod pending_objects;
mod read_capabilities;
pub mod read_snapshot;
pub mod schema;
pub mod snapshot;
pub mod source_snapshot;
pub mod sync_entities;
pub mod sync_kv;
pub mod sync_tombstones;
pub mod sync_versions;
pub mod task_merge;
pub mod task_mutation;
mod tracked_app_mutation;
pub mod type_registry;
pub mod usage;
pub mod usage_history;
pub mod usage_mutation;
pub mod usage_queries;
pub mod usage_sync_log;
pub mod writer;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use read_capabilities::{
    CanonicalReadCapabilities, IntegrationReplicationReadCapabilities, ObjectReadCapabilities,
    ReadCapabilities, SystemReadCapabilities, TypeRegistryReadCapabilities, UsageReadCapabilities,
};

pub const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";

#[cfg(test)]
mod canonical_facades_tests;
#[cfg(test)]
mod canonical_game_tests;
#[cfg(test)]
mod canonical_type_bootstrap_tests;
#[cfg(test)]
mod core_storage_tests;
#[cfg(test)]
mod data_platform_tests;
#[cfg(test)]
mod integration_schema_upgrade_tests;
#[cfg(test)]
mod load_all_json_tests;
#[cfg(test)]
mod load_and_clear_tests;
#[cfg(test)]
mod pending_objects_and_integrity_tests;
#[cfg(test)]
mod phase2_object_versions_tests;
#[cfg(test)]
mod query_contract_tests;
#[cfg(test)]
mod schema_migration_contract_tests;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod sync_storage_contract_tests;
#[cfg(test)]
mod sync_transaction_contract_tests;
#[cfg(test)]
mod transactions_and_usage_tests;
#[cfg(test)]
mod type_registry_slice1_tests;
