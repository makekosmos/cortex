use super::*;
use ark_application::sync_storage::InboxReceiptWrite;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for SqliteStorageBackend {
    async fn receive_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        let payload_json = serde_json::to_string(entity).map_err(|error| error.to_string())?;
        let context = CommandContext::authenticated_peer(authenticated_peer_id.to_owned());
        let write = InboxReceiptWrite::new(
            authenticated_peer_id.to_owned(),
            message_id.to_owned(),
            payload_json,
        );
        self.db
            .receive_inbox_receipt(context, write)
            .await
            .map(|state| state != crate::db::InboxState::Rejected)
            .map_err(|error| error.to_string())
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        local_device_id: &str,
        policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    ) -> Result<bool, String> {
        let payload_json = serde_json::to_string(entity).map_err(|error| error.to_string())?;
        let write = match policy_identity {
            Some(identity) => InboxReceiptWrite::with_policy(
                authenticated_peer_id.to_owned(),
                message_id.to_owned(),
                payload_json,
                local_device_id.to_owned(),
                identity,
            ),
            None => InboxReceiptWrite::new(
                authenticated_peer_id.to_owned(),
                message_id.to_owned(),
                payload_json,
            ),
        };
        let context = CommandContext::authenticated_peer(authenticated_peer_id.to_owned());
        self.db
            .receive_inbox_receipt(context, write)
            .await
            .map(|state| state != crate::db::InboxState::Rejected)
            .map_err(|error| error.to_string())
    }

    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        let inbox_message_id = ark_application::sync_storage::entity_inbox_message_id(
            authenticated_peer_id,
            &entity.entity_type,
            &entity.id,
            &entity.hlc,
        );
        self.apply_authenticated_entity_with_id(entity, authenticated_peer_id, &inbox_message_id)
            .await
            .map(|_| ())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        inbox_message_id: &str,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        self.apply_authenticated_entity_with_policy(
            entity,
            authenticated_peer_id,
            inbox_message_id,
            "",
            None,
        )
        .await
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        inbox_message_id: &str,
        local_device_id: &str,
        policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        let inbox_payload_json =
            serde_json::to_string(entity).map_err(|error| error.to_string())?;
        let context = CommandContext::authenticated_peer(authenticated_peer_id.to_owned());
        if is_full_core_task(entity) {
            let receipt_command = make_inbox_receipt_write(
                authenticated_peer_id,
                inbox_message_id,
                inbox_payload_json.clone(),
                local_device_id,
                policy_identity.as_ref(),
            );
            let inbox_state = self
                .db
                .receive_inbox_receipt(context.clone(), receipt_command)
                .await
                .map_err(|error| error.to_string())?;
            if inbox_state == crate::db::InboxState::Applied {
                return Ok(ark_application::sync_storage::InboundApplyOutcome::Applied);
            }
            if inbox_state == crate::db::InboxState::Pending {
                return Ok(ark_application::sync_storage::InboundApplyOutcome::Pending);
            }
            let mut command = AuthenticatedCoreTaskWrite::new(
                entity.id.clone(),
                entity.hlc.clone(),
                entity.origin_device_id.clone(),
                entity.data.clone(),
                inbox_message_id.to_owned(),
                inbox_payload_json,
            );
            if let Some(identity) = policy_identity {
                command = command.with_policy_identity(local_device_id.to_owned(), identity);
            }
            return self
                .db
                .submit_authenticated_core_task_write(
                    CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                    command,
                )
                .await
                .map(|outcome| match outcome {
                    crate::db::RemoteCoreTaskOutcome::Applied
                    | crate::db::RemoteCoreTaskOutcome::Duplicate => {
                        ark_application::sync_storage::InboundApplyOutcome::Applied
                    }
                    crate::db::RemoteCoreTaskOutcome::Stale => {
                        ark_application::sync_storage::InboundApplyOutcome::Stale
                    }
                })
                .map_err(|error| error.to_string());
        }
        let write = make_inbox_receipt_write(
            authenticated_peer_id,
            inbox_message_id,
            inbox_payload_json,
            local_device_id,
            policy_identity.as_ref(),
        );
        let result = self
            .db
            .submit_authenticated_inbound_entity(context, write, entity.clone())
            .await
            .map_err(|error| error.to_string())?;
        if let Some(type_id) = result.pending_type_id {
            crate::events::emit_event(serde_json::json!({
                "event": "sync_error", "code": "unknown_type_version", "entity_type": "object",
                "entity_id": entity.id, "awaited_type_id": type_id,
                "awaited_type_version": entity
                    .data
                    .get("typeVersion")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("0.0.0-legacy"),
            }));
        }
        publish_replay_notices(result.replay_notices);
        Ok(result.outcome)
    }
}

fn make_inbox_receipt_write(
    peer_id: &str,
    message_id: &str,
    payload_json: String,
    local_device_id: &str,
    policy_identity: Option<&ark_application::sync_storage::PolicyIdentity>,
) -> InboxReceiptWrite {
    match policy_identity {
        Some(identity) => InboxReceiptWrite::with_policy(
            peer_id.to_owned(),
            message_id.to_owned(),
            payload_json,
            local_device_id.to_owned(),
            identity.clone(),
        ),
        None => InboxReceiptWrite::new(peer_id.to_owned(), message_id.to_owned(), payload_json),
    }
}
