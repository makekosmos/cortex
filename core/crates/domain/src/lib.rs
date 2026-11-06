#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod app_identity;
pub mod canonical_game;
pub mod canonical_type;
pub mod device_registry;
pub mod hlc;
pub mod load_all_data;
pub mod object_read;
pub mod object_write;
pub mod registry_admission;
pub mod replication_storage;
pub mod selective_sync;
pub mod sync_contracts;
pub mod sync_policy;
pub mod sync_storage;
pub mod task_merge;
pub mod task_merge_sync;
pub mod type_registry;
pub mod types;
pub use sync_policy::PolicyIdentity;
pub use task_merge_sync::{
    TaskMergeBatch, TaskMergeBatchAck, TaskMergeJournalEntry, TaskMergeJournalRecord,
    MAX_TASK_MERGE_PAGE_SIZE,
};
