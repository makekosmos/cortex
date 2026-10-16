use super::*;

pub(super) async fn accept(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    authenticated: bool,
    revision: i64,
    digest: &str,
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
        .accept_peer_sync_policy(&session.peer_id, revision, digest, &rules)
        .await
        .map(|_| ())
}
