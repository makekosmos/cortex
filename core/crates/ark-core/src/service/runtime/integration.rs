use super::*;

pub(super) async fn integration_persist_node_authorization(
    state: &Arc<ServiceState>,
    authorization_operation: String,
    node: AuthorizedNode,
    grant: Option<IntegrationNodeGrant>,
    device_id: String,
) -> Result<Value, String> {
    let operation = parse_authorization_operation(&authorization_operation)?;
    get_db_handle(state)?
        .submit_integration_admin(
            db::CommandContext::engine(),
            db::IntegrationAdminCommand::PersistNodeAuthorization {
                operation,
                node,
                grant,
                device_id,
            },
        )
        .await
        .and_then(require_integration_admin_complete)
        .map(|()| json!(true))
        .map_err(|error| error.to_string())
}

pub(super) async fn integration_persist_integration_grant(
    state: &Arc<ServiceState>,
    grant: IntegrationNodeGrant,
    device_id: String,
) -> Result<Value, String> {
    get_db_handle(state)?
        .submit_integration_admin(
            db::CommandContext::engine(),
            db::IntegrationAdminCommand::PersistGrant { grant, device_id },
        )
        .await
        .and_then(require_integration_admin_complete)
        .map(|()| json!(true))
        .map_err(|error| error.to_string())
}

pub(super) async fn integration_prepare_signed_sync(
    state: &Arc<ServiceState>,
    space_id: String,
    origin_node_id: String,
    integration_id: String,
    recipient_node_id: String,
    message_id: String,
) -> Result<Value, String> {
    integration_read(
        state,
        db::IntegrationReadQuery::PrepareSignedSync {
            space_id,
            origin_node_id,
            integration_id,
            recipient_node_id,
            message_id,
        },
    )
    .await
}

pub(super) async fn integration_validate_outbound_signed_sync(
    state: &Arc<ServiceState>,
    space_id: String,
    origin_node_id: String,
    frame: SignedSyncEnvelope,
) -> Result<Value, String> {
    integration_read(
        state,
        db::IntegrationReadQuery::ValidateOutbound {
            space_id,
            origin_node_id,
            frame,
        },
    )
    .await
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
    integration_read(
        state,
        db::IntegrationReadQuery::ValidateOutbound {
            space_id: runtime.space_id.clone(),
            origin_node_id: runtime.device_id.clone(),
            frame: frame.clone(),
        },
    )
    .await?;
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

pub(super) async fn integration_acquire_refresh_lease(
    state: &Arc<ServiceState>,
    params: crate::db::RefreshLeaseAcquireParams,
) -> Result<Value, String> {
    let lease = get_db_handle(state)?
        .submit_integration_admin(
            db::CommandContext::engine(),
            db::IntegrationAdminCommand::AcquireRefreshLease(params),
        )
        .await
        .map_err(|error| error.to_string())?;
    let db::IntegrationAdminOutput::Lease(lease) = lease else {
        return Err("integration refresh operation returned an invalid result".into());
    };
    serde_json::to_value(lease).map_err(|error| error.to_string())
}

pub(super) async fn integration_publish_credential_envelope(
    state: &Arc<ServiceState>,
    envelope: IntegrationCredentialEnvelope,
    device_id: String,
    now_ms: u64,
) -> Result<Value, String> {
    get_db_handle(state)?
        .submit_integration_admin(
            db::CommandContext::engine(),
            db::IntegrationAdminCommand::PublishCredentialEnvelope {
                envelope,
                device_id,
                now_ms,
            },
        )
        .await
        .and_then(require_integration_admin_complete)
        .map(|()| json!(true))
        .map_err(|error| error.to_string())
}

fn require_integration_admin_complete(
    output: db::IntegrationAdminOutput,
) -> Result<(), crate::db::DbError> {
    match output {
        db::IntegrationAdminOutput::Complete => Ok(()),
        db::IntegrationAdminOutput::Lease(_) => Err(crate::db::DbError::Invalid(
            "integration administration returned an unexpected lease".into(),
        )),
    }
}

pub(super) async fn integration_load_latest_credential_envelope(
    state: &Arc<ServiceState>,
    integration_id: String,
    recipient_node_id: String,
) -> Result<Value, String> {
    integration_read(
        state,
        db::IntegrationReadQuery::LatestCredentialEnvelope {
            integration_id,
            recipient_node_id,
        },
    )
    .await
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
    integration_read(
        state,
        db::IntegrationReadQuery::IssuerEncryptionKey {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            credential_generation,
            expected_issuer_key_id,
        },
    )
    .await
}

/// Storage-time fence check for a received credential envelope (KOS-270).
/// `expected` already carries every field so this stays under the arg limit.
pub(super) async fn integration_check_credential_fence(
    state: &Arc<ServiceState>,
    expected: crate::db::CredentialFenceExpectation<'_>,
) -> Result<Value, String> {
    let runtime_space_id = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|runtime| runtime.space_id.clone())
        .ok_or_else(|| "sync not running".to_string())?;
    if runtime_space_id != expected.space_id {
        return Err("integration lookup requested for the wrong space".into());
    }
    integration_read(
        state,
        db::IntegrationReadQuery::CredentialFence {
            space_id: expected.space_id.to_owned(),
            integration_id: expected.integration_id.to_owned(),
            recipient_node_id: expected.recipient_node_id.to_owned(),
            issuer_node_id: expected.issuer_node_id.to_owned(),
            credential_generation: expected.credential_generation,
            refresh_fencing_token: expected.refresh_fencing_token,
            expected_issuer_key_id: expected.expected_issuer_key_id.to_owned(),
        },
    )
    .await
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
    integration_read(
        state,
        db::IntegrationReadQuery::IssuerEncryptionKeyForPublish {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            expected_issuer_key_id,
        },
    )
    .await
}

pub(super) async fn integration_verification_status(
    state: &Arc<ServiceState>,
    integration_id: String,
    local_node_id: String,
    now_ms: u64,
) -> Result<Value, String> {
    integration_read(
        state,
        db::IntegrationReadQuery::VerificationStatus {
            integration_id,
            local_node_id,
            now_ms,
        },
    )
    .await
}

async fn integration_read(
    state: &ServiceState,
    query: db::IntegrationReadQuery,
) -> Result<Value, String> {
    get_db_handle(state)?
        .query_with_store(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
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
