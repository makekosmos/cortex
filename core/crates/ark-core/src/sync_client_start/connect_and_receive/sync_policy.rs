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
                .contains(&ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        })
}

pub(super) async fn validate_snapshot_identity(
    storage: &Arc<dyn SyncRuntimeStorage>,
    local_device_id: &str,
    policy_bound: bool,
    received: &Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let expected = if policy_bound {
        let profile = storage.load_local_sync_profile(local_device_id).await?;
        Some(ark_sync::protocol::sync_policy_identity(profile.as_ref()))
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
    rules: Vec<ark_sync::protocol::SyncPolicyRuleWire>,
) -> Result<(), String> {
    if !authenticated {
        return Err("sync policy arrived before authentication".into());
    }
    let session = authenticated_tx
        .lock()
        .await
        .clone()
        .ok_or_else(|| "sync policy has no authenticated peer session".to_owned())?;
    ark_sync::client_session::accept_sync_policy(
        storage.as_ref(),
        &session.peer_id,
        &session.protocol,
        authenticated,
        ark_sync::client_session::SyncPolicyUpdate {
            revision,
            digest: digest.to_owned(),
            active,
            rules,
        },
    )
    .await
}

pub(super) async fn accept_and_start(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    authenticated: bool,
    revision: i64,
    digest: &str,
    active: bool,
    rules: Vec<ark_sync::protocol::SyncPolicyRuleWire>,
) -> Result<(), String> {
    accept(
        storage,
        authenticated_tx,
        authenticated,
        revision,
        digest,
        active,
        rules,
    )
    .await?;
    start_sync_after_policy(storage, authenticated_tx).await
}

pub(super) async fn start_sync_after_policy(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
) -> Result<(), String> {
    let (peer_id, tx) = {
        let mut session = authenticated_tx.lock().await;
        let session = session
            .as_mut()
            .ok_or_else(|| "sync policy has no authenticated peer session".to_owned())?;
        if session.sync_started {
            return Ok(());
        }
        session.sync_started = true;
        (session.peer_id.clone(), session.tx.clone())
    };
    let vector = ark_sync::client_session::begin_initial_sync(storage.as_ref(), &peer_id).await?;
    send_msg(
        &tx,
        LanSyncMessage::VersionVector {
            vector,
            origin_device_id: None,
        },
    )
    .await?;
    Ok(())
}
