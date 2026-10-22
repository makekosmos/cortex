#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod app_identity;
pub mod canonical_type;
pub mod device_registry;
pub mod hlc;
pub mod integration_replication;
pub mod object_write;
pub mod selective_sync;
pub mod sync_policy;
pub mod task_merge;
pub mod task_merge_sync;
pub mod types;
pub use sync_policy::PolicyIdentity;
pub use task_merge_sync::{
    TaskMergeBatch, TaskMergeBatchAck, TaskMergeJournalEntry, TaskMergeJournalRecord,
    MAX_TASK_MERGE_PAGE_SIZE,
};
