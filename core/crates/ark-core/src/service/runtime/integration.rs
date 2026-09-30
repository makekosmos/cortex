use super::*;

pub(super) async fn integration_persist_node_authorization(
    state: &Arc<ServiceState>,
    authorization_operation: String,
    node: AuthorizedNode,
    grant: Option<IntegrationNodeGrant>,
    device_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let operation = parse_authorization_operation(&authorization_operation)?;
        crate::integration_replication::persist_node_authorization(
            conn,
            operation,
            &node,
            grant.as_ref(),
            &device_id,
        )?;
        Ok(json!(true))
    })
}

pub(super) async fn integration_persist_integration_grant(
    state: &Arc<ServiceState>,
    grant: IntegrationNodeGrant,
    device_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        crate::integration_replication::persist_integration_grant(conn, &grant, &device_id)?;
        Ok(json!(true))
    })
}

pub(super) async fn integration_prepare_signed_sync(
    state: &Arc<ServiceState>,
    space_id: String,
    origin_node_id: String,
    integration_id: String,
    recipient_node_id: String,
    message_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let preparation = crate::integration_replication::prepare_signed_sync(
            conn,
            &space_id,
            &origin_node_id,
            &integration_id,
            &recipient_node_id,
            &message_id,
        )?;
        serde_json::to_value(preparation).map_err(|error| error.to_string())
    })
}

pub(super) async fn integration_validate_outbound_signed_sync(
    state: &Arc<ServiceState>,
    space_id: String,
    origin_node_id: String,
    frame: SignedSyncEnvelope,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        crate::integration_replication::validate_outbound_signed_sync(
            conn,
            &space_id,
            &origin_node_id,
            &frame,
        )?;
        Ok(json!(true))
    })
}

pub(super) async fn integration_send_signed_sync(
    state: &Arc<ServiceState>,
    frame: SignedSyncEnvelope,
) -> Result<Value, String> {
    let runtime = state
        .sync
        .lock()
        .await
        .as_ref()
        .cloned()
        .ok_or_else(|| "sync not running".to_string())?;
    with_conn(state, |conn| {
        crate::integration_replication::validate_outbound_signed_sync(
            conn,
            &runtime.space_id,
            &runtime.device_id,
            &frame,
        )
    })?;
    if let Some(relay) = runtime.relay.as_ref() {
        if relay
            .send_signed_integration_frame(frame.clone())
            .await
            .is_ok()
        {
            return Ok(json!(true));
        }
    }
    if runtime
        .server
        .send_signed_integration_frame(&frame.recipient_node_id, frame.clone())
        .await
        .is_ok()
    {
        return Ok(json!(true));
    }
    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        if client
            .send_signed_integration_frame(frame.clone())
            .await
            .is_ok()
        {
            return Ok(json!(true));
        }
    }
    if let Some(relay) = runtime.relay.as_ref() {
        if relay.send_signed_integration_frame(frame).await.is_ok() {
            return Ok(json!(true));
        }
    }
    Err("target peer has no authenticated addressed route".into())
}

// Mirrors the IntegrationAcquireRefreshLease request fields one-to-one; the
// request struct is the params object already.
#[allow(clippy::too_many_arguments)]
pub(super) async fn integration_acquire_refresh_lease(
    state: &Arc<ServiceState>,
    integration_id: String,
    holder_node_id: String,
    credential_generation: u64,
    now_ms: u64,
    ttl_ms: u64,
    expected_fencing_token: u64,
    device_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let lease = crate::db::try_acquire_integration_refresh_lease(
            conn,
            &integration_id,
            &holder_node_id,
            credential_generation,
            now_ms,
            ttl_ms,
            expected_fencing_token,
            &device_id,
        )?;
        serde_json::to_value(lease).map_err(|error| error.to_string())
    })
}

pub(super) async fn integration_publish_credential_envelope(
    state: &Arc<ServiceState>,
    envelope: IntegrationCredentialEnvelope,
    device_id: String,
    now_ms: u64,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        crate::db::publish_integration_credential_envelope(conn, &envelope, &device_id, now_ms)?;
        Ok(json!(true))
    })
}

pub(super) async fn integration_load_latest_credential_envelope(
    state: &Arc<ServiceState>,
    integration_id: String,
    recipient_node_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let envelope = crate::db::load_latest_integration_credential_envelope(
            conn,
            &integration_id,
            &recipient_node_id,
        )?;
        serde_json::to_value(envelope).map_err(|error| error.to_string())
    })
}

pub(super) async fn integration_lookup_issuer_encryption_key(
    state: &Arc<ServiceState>,
    space_id: String,
    integration_id: String,
    recipient_node_id: String,
    issuer_node_id: String,
    credential_generation: u64,
    expected_issuer_key_id: String,
) -> Result<Value, String> {
    let runtime_space_id = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|runtime| runtime.space_id.clone())
        .ok_or_else(|| "sync not running".to_string())?;
    if runtime_space_id != space_id {
        return Err("integration lookup requested for the wrong space".into());
    }
    with_conn(state, |conn| {
        let key = crate::db::load_issuer_encryption_key(
            conn,
            &space_id,
            &integration_id,
            &recipient_node_id,
            &issuer_node_id,
            credential_generation,
            &expected_issuer_key_id,
        )?;
        serde_json::to_value(key).map_err(|error| error.to_string())
    })
}

pub(super) async fn integration_lookup_issuer_encryption_key_for_publish(
    state: &Arc<ServiceState>,
    space_id: String,
    integration_id: String,
    recipient_node_id: String,
    issuer_node_id: String,
    expected_issuer_key_id: String,
) -> Result<Value, String> {
    let runtime_space_id = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|runtime| runtime.space_id.clone())
        .ok_or_else(|| "sync not running".to_string())?;
    if runtime_space_id != space_id {
        return Err("integration lookup requested for the wrong space".into());
    }
    with_conn(state, |conn| {
        let key = crate::db::load_issuer_encryption_key_for_publish(
            conn,
            &space_id,
            &integration_id,
            &recipient_node_id,
            &issuer_node_id,
            &expected_issuer_key_id,
        )?;
        serde_json::to_value(key).map_err(|error| error.to_string())
    })
}

pub(super) async fn integration_verification_status(
    state: &Arc<ServiceState>,
    integration_id: String,
    local_node_id: String,
    now_ms: u64,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let status = crate::db::integration_verification_status(
            conn,
            &integration_id,
            &local_node_id,
            now_ms,
        )?;
        serde_json::to_value(status).map_err(|error| error.to_string())
    })
}

fn parse_authorization_operation(
    value: &str,
) -> Result<crate::integration_replication::NodeAuthorizationOperation, String> {
    match value {
        "authorize" => Ok(crate::integration_replication::NodeAuthorizationOperation::Authorize),
        "revoke" => Ok(crate::integration_replication::NodeAuthorizationOperation::Revoke),
        "rotate" => Ok(crate::integration_replication::NodeAuthorizationOperation::Rotate),
        _ => Err("authorization_operation must be authorize, revoke, or rotate".into()),
    }
}
