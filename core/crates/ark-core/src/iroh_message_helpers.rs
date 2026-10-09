// ---------------------------------------------------------------------------
// Diagnostics helpers
// ---------------------------------------------------------------------------

/// Returns a short, non-allocating variant name for logging (BUG 3).
fn message_variant_name(msg: &LanSyncMessage) -> &'static str {
    match msg {
        LanSyncMessage::Hello { .. } => "Hello",
        LanSyncMessage::VersionVector { .. } => "VersionVector",
        LanSyncMessage::SyncChanges { .. } => "SyncChanges",
        LanSyncMessage::SyncAck { .. } => "SyncAck",
        LanSyncMessage::LiveChange { .. } => "LiveChange",
        LanSyncMessage::LiveAck { .. } => "LiveAck",
        LanSyncMessage::PeerList { .. } => "PeerList",
        LanSyncMessage::Ping { .. } => "Ping",
        LanSyncMessage::Pong { .. } => "Pong",
        LanSyncMessage::SignedIntegrationFrame { .. } => "SignedIntegrationFrame",
        LanSyncMessage::SignedIntegrationAck { .. } => "SignedIntegrationAck",
        LanSyncMessage::PairingRejected { .. } => "PairingRejected",
    }
}

// ---------------------------------------------------------------------------
