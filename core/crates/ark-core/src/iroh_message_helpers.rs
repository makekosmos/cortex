// ---------------------------------------------------------------------------
// Diagnostics helpers
// ---------------------------------------------------------------------------

/// Returns a short, non-allocating variant name for logging (BUG 3).
fn message_variant_name(msg: &LanSyncMessage) -> &'static str {
    match msg {
        LanSyncMessage::Hello { .. } => "Hello",
        LanSyncMessage::VersionVector { .. } => "VersionVector",
        LanSyncMessage::SyncPolicy { .. } => "SyncPolicy",
        LanSyncMessage::SyncChanges { .. } => "SyncChanges",
        LanSyncMessage::SyncAck { .. } => "SyncAck",
        LanSyncMessage::LiveChange { .. } => "LiveChange",
        LanSyncMessage::LiveAck { .. } => "LiveAck",
        LanSyncMessage::LiveReceiptAck { .. } => "LiveReceiptAck",
        LanSyncMessage::TaskMergeBatch { .. } => "TaskMergeBatch",
        LanSyncMessage::TaskMergeBatchAck { .. } => "TaskMergeBatchAck",
        LanSyncMessage::ProtocolRejected { .. } => "ProtocolRejected",
        LanSyncMessage::PeerList { .. } => "PeerList",
        LanSyncMessage::Ping { .. } => "Ping",
        LanSyncMessage::Pong { .. } => "Pong",
        LanSyncMessage::SignedIntegrationFrame { .. } => "SignedIntegrationFrame",
        LanSyncMessage::SignedIntegrationAck { .. } => "SignedIntegrationAck",
        LanSyncMessage::PairingRejected { .. } => "PairingRejected",
        LanSyncMessage::PairingAccepted { .. } => "PairingAccepted",
        LanSyncMessage::RegistryAdmissionStart { .. } => "RegistryAdmissionStart",
        LanSyncMessage::RegistryAdmissionProof { .. } => "RegistryAdmissionProof",
        LanSyncMessage::RegistryAdmissionConfirm { .. } => "RegistryAdmissionConfirm",
        LanSyncMessage::RegistryEventsPage { .. } => "RegistryEventsPage",
        LanSyncMessage::RegistrySyncStart { .. } => "RegistrySyncStart",
        LanSyncMessage::RegistrySyncReady { .. } => "RegistrySyncReady",
        LanSyncMessage::RegistryEventUpdate { .. } => "RegistryEventUpdate",
    }
}

// ---------------------------------------------------------------------------
