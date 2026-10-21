mod sync_policy;
mod vector;
pub use sync_policy::{
    default_sync_policy_message, parse_sync_policy_rules, sync_policy_identity, sync_policy_message,
};
pub use vector::{
    apply_usage_complete_through, compute_local_excess, compute_vector_diff, is_usage_entity,
    merge_peer_records, merge_usage_cursors, merge_vector_entries, observe_non_usage_entity,
    should_send_entity, split_into_batches,
};

#[cfg(test)]
include!("protocol/tests.rs");
