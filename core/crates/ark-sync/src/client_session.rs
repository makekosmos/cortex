use ark_application::{
    replication_storage::PeerIdentityBackend,
    sync_storage::{CommittedFeedPort, PeerProgressPort},
};
use ark_domain::types::VersionVector;

use crate::protocol::{NegotiatedProtocol, ProtocolCapability, SyncPolicyRuleWire};

pub struct SyncPolicyUpdate {
    pub revision: i64,
    pub digest: String,
    pub active: bool,
    pub rules: Vec<SyncPolicyRuleWire>,
}

pub async fn accept_sync_policy<S: PeerIdentityBackend + ?Sized>(
    storage: &S,
    peer_device_id: &str,
    protocol: &NegotiatedProtocol,
    authenticated: bool,
    update: SyncPolicyUpdate,
) -> Result<(), String> {
    if !authenticated {
        return Err("sync policy arrived before authentication".into());
    }
    if !protocol
        .capabilities
        .contains(&ProtocolCapability::SelectiveSyncPolicyV1)
    {
        return Err("sync policy capability was not negotiated".into());
    }

    let rules = crate::protocol::parse_sync_policy_rules(update.rules)?;
    storage
        .accept_peer_sync_policy(
            peer_device_id,
            update.revision,
            &update.digest,
            update.active,
            &rules,
        )
        .await
        .map(|_| ())
}

pub async fn begin_initial_sync<S>(
    storage: &S,
    peer_device_id: &str,
) -> Result<VersionVector, String>
where
    S: CommittedFeedPort + PeerProgressPort + ?Sized,
{
    let generation = storage
        .selective_sync_policy_generation_for_peer(peer_device_id)
        .await?;
    storage
        .begin_outbound_policy(peer_device_id, generation)
        .await?;
    storage
        .load_version_vector()
        .await
        .map_err(|error| format!("failed to load sync version vector: {error}"))
}
