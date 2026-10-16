use super::*;

pub(super) async fn is_policy_bound(
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
) -> bool {
    authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        })
}

pub(super) async fn validate_snapshot_identity(
    storage: &Arc<dyn SyncRuntimeStorage>,
    local_device_id: &str,
    policy_bound: bool,
    received: &Option<crate::application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let expected = if policy_bound {
        let profile = storage.load_local_sync_profile(local_device_id).await?;
        Some(crate::protocol::sync_policy_identity(profile.as_ref()))
    } else {
        None
    };
    if received == &expected {
        Ok(())
    } else {
        Err("stale_or_unnegotiated_snapshot_policy".into())
    }
}

pub(super) async fn accept(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    authenticated: bool,
    revision: i64,
    digest: &str,
    active: bool,
    rules: Vec<crate::protocol::SyncPolicyRuleWire>,
) -> Result<(), String> {
    if !authenticated {
        return Err("sync policy arrived before authentication".into());
    }
    let session = authenticated_tx
        .lock()
        .await
        .clone()
        .ok_or_else(|| "sync policy has no authenticated peer session".to_owned())?;
    if !session
        .protocol
        .capabilities
        .contains(&crate::protocol::ProtocolCapability::SelectiveSyncPolicyV1)
    {
        return Err("sync policy capability was not negotiated".into());
    }
    let rules = crate::protocol::parse_sync_policy_rules(rules)?;
    storage
        .accept_peer_sync_policy(&session.peer_id, revision, digest, active, &rules)
        .await
        .map(|_| ())
}
