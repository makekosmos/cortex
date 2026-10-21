use serde::{Deserialize, Serialize};

use ark_domain::types::*;

mod sync_policy;
mod vector;
use sync_policy::default_sync_policy_active;

pub use crate::protocol_negotiation::*;
pub use sync_policy::{
    default_sync_policy_message, parse_sync_policy_rules, sync_policy_identity,
    sync_policy_message, SyncPolicyRuleWire,
};
pub use vector::{
    apply_usage_complete_through, compute_local_excess, compute_vector_diff, is_usage_entity,
    merge_peer_records, merge_usage_cursors, merge_vector_entries, observe_non_usage_entity,
    should_send_entity, split_into_batches,
};

include!("protocol/messages.rs");

/// Whether a frame carries replicated or peer-visible data and therefore must
/// wait for authenticated registry admission before a transport sends it.
pub fn requires_registry_admission(message: &LanSyncMessage) -> bool {
    matches!(
        message,
        LanSyncMessage::VersionVector { .. }
            | LanSyncMessage::SyncChanges { .. }
            | LanSyncMessage::SyncPolicy { .. }
            | LanSyncMessage::SyncAck { .. }
            | LanSyncMessage::LiveChange { .. }
            | LanSyncMessage::LiveAck { .. }
            | LanSyncMessage::LiveReceiptAck { .. }
            | LanSyncMessage::TaskMergeBatch { .. }
            | LanSyncMessage::TaskMergeBatchAck { .. }
            | LanSyncMessage::SignedIntegrationFrame { .. }
            | LanSyncMessage::SignedIntegrationAck { .. }
            | LanSyncMessage::PeerList { .. }
    )
}

#[cfg(test)]
include!("protocol/tests.rs");
