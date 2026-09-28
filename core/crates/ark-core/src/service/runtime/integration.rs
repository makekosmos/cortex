use super::*;

pub(super) async fn handle(state: &Arc<ServiceState>, request: Request) -> Result<Value, String> {
    match request {
        Request::IntegrationPersistNodeAuthorization {
            authorization_operation,
            node,
            grant,
            device_id,
        } => with_conn(state, |conn| {
            let operation = parse_authorization_operation(&authorization_operation)?;
            crate::integration_replication::persist_node_authorization(
                conn,
                operation,
                &node,
                grant.as_ref(),
                &device_id,
            )?;
            Ok(json!(true))
        }),
        Request::IntegrationPersistIntegrationGrant { grant, device_id } => {
            with_conn(state, |conn| {
                crate::integration_replication::persist_integration_grant(
                    conn, &grant, &device_id,
                )?;
                Ok(json!(true))
            })
        }
        Request::IntegrationPrepareSignedSync {
            space_id,
            origin_node_id,
            integration_id,
            recipient_node_id,
            message_id,
        } => with_conn(state, |conn| {
            let preparation = crate::integration_replication::prepare_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &integration_id,
                &recipient_node_id,
                &message_id,
            )?;
            serde_json::to_value(preparation).map_err(|error| error.to_string())
        }),
        Request::IntegrationValidateOutboundSignedSync {
            space_id,
            origin_node_id,
            frame,
        } => with_conn(state, |conn| {
            crate::integration_replication::validate_outbound_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &frame,
            )?;
            Ok(json!(true))
        }),
        Request::IntegrationSendSignedSync { frame } => {
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
        Request::IntegrationAcquireRefreshLease {
            integration_id,
            holder_node_id,
            credential_generation,
            now_ms,
            ttl_ms,
            expected_fencing_token,
            device_id,
        } => with_conn(state, |conn| {
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
        }),
        Request::IntegrationPublishCredentialEnvelope {
            envelope,
            device_id,
            now_ms,
        } => with_conn(state, |conn| {
            crate::db::publish_integration_credential_envelope(
                conn, &envelope, &device_id, now_ms,
            )?;
            Ok(json!(true))
        }),
        Request::IntegrationLoadLatestCredentialEnvelope {
            integration_id,
            recipient_node_id,
        } => with_conn(state, |conn| {
            let envelope = crate::db::load_latest_integration_credential_envelope(
                conn,
                &integration_id,
                &recipient_node_id,
            )?;
            serde_json::to_value(envelope).map_err(|error| error.to_string())
        }),
        Request::IntegrationLookupIssuerEncryptionKey {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            credential_generation,
            expected_issuer_key_id,
        } => {
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
        Request::IntegrationLookupIssuerEncryptionKeyForPublish {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            expected_issuer_key_id,
        } => {
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
        Request::IntegrationVerificationStatus {
            integration_id,
            local_node_id,
            now_ms,
        } => with_conn(state, |conn| {
            let status = crate::db::integration_verification_status(
                conn,
                &integration_id,
                &local_node_id,
                now_ms,
            )?;
            serde_json::to_value(status).map_err(|error| error.to_string())
        }),
        _ => Err("request is not an integration replication operation".into()),
    }
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
