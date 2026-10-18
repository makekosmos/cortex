use super::*;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for SqliteStorageBackend {
    async fn receive_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        let payload_json = serde_json::to_string(entity).map_err(|error| error.to_string())?;
        self.db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                crate::db::ReceiveInboxReceiptCommand::new(
                    authenticated_peer_id.to_owned(),
                    message_id.to_owned(),
                    payload_json,
                ),
            )
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
        let command = match policy_identity {
            Some(identity) => crate::db::ReceiveInboxReceiptCommand::with_policy_identity(
                authenticated_peer_id.to_owned(),
                message_id.to_owned(),
                payload_json,
                local_device_id.to_owned(),
                identity,
            ),
            None => crate::db::ReceiveInboxReceiptCommand::new(
                authenticated_peer_id.to_owned(),
                message_id.to_owned(),
                payload_json,
            ),
        };
        self.db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                command,
            )
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
        let receipt_command = match policy_identity.as_ref() {
            Some(identity) => crate::db::ReceiveInboxReceiptCommand::with_policy_identity(
                authenticated_peer_id.to_owned(),
                inbox_message_id.to_owned(),
                inbox_payload_json.clone(),
                local_device_id.to_owned(),
                identity.clone(),
            ),
            None => crate::db::ReceiveInboxReceiptCommand::new(
                authenticated_peer_id.to_owned(),
                inbox_message_id.to_owned(),
                inbox_payload_json.clone(),
            ),
        };
        let inbox_state = self
            .db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                receipt_command,
            )
            .await
            .map_err(|error| error.to_string())?;
        if inbox_state == crate::db::InboxState::Applied {
            return Ok(ark_application::sync_storage::InboundApplyOutcome::Applied);
        }
        if inbox_state == crate::db::InboxState::Pending {
            return Ok(ark_application::sync_storage::InboundApplyOutcome::Pending);
        }
        if is_full_core_task(entity) {
            let command = match policy_identity.clone() {
                Some(identity) => AuthenticatedCoreTaskWrite::with_policy_identity(
                    entity.id.clone(),
                    entity.hlc.clone(),
                    entity.origin_device_id.clone(),
                    entity.data.clone(),
                    inbox_message_id.to_owned(),
                    inbox_payload_json,
                    local_device_id.to_owned(),
                    identity,
                ),
                None => AuthenticatedCoreTaskWrite::new(
                    entity.id.clone(),
                    entity.hlc.clone(),
                    entity.origin_device_id.clone(),
                    entity.data.clone(),
                    inbox_message_id.to_owned(),
                    inbox_payload_json,
                ),
            };
            return self
                .db
                .submit(
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
        let (outcome, notices) = self
            .db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                match policy_identity {
                    Some(identity) => {
                        crate::db::ApplyAuthenticatedSyncEntityCommand::with_policy_identity(
                            entity.clone(),
                            inbox_message_id.to_owned(),
                            local_device_id.to_owned(),
                            identity,
                        )
                    }
                    None => crate::db::ApplyAuthenticatedSyncEntityCommand::with_message_id(
                        entity.clone(),
                        inbox_message_id.to_owned(),
                    ),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_replay_notices(notices);
        Ok(outcome)
    }
}
