mod messages;
pub use messages::*;

pub const fn default_sync_policy_active() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPolicyRuleWire {
    pub resource_kind: String,
    pub resource_id: String,
    pub mode: ark_domain::selective_sync::SyncMode,
    pub filter: serde_json::Value,
}

/// Whether a frame carries replicated or peer-visible data and must wait for
/// authenticated registry admission.
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
