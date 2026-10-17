use ark_domain::types::SyncEntity;

/// Applies an ordinary entity only after the sync boundary authenticated its peer.
#[async_trait::async_trait]
pub trait AuthenticatedEntityBackend: Send + Sync {
    /// Commits the bounded inbox receipt before applying the payload. Returns
    /// `true` only when this backend provides that durable boundary.
    async fn receive_authenticated_entity_with_id(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        _local_device_id: &str,
        policy_identity: Option<crate::sync_storage::PolicyIdentity>,
    ) -> Result<bool, String> {
        if policy_identity.is_some() {
            return Err("policy_bound_receipt_unsupported".into());
        }
        self.receive_authenticated_entity_with_id(entity, authenticated_peer_id, message_id)
            .await
    }

    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Err("authenticated_apply_unsupported".into())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String> {
        self.apply_authenticated_entity(entity, authenticated_peer_id)
            .await
            .map(|()| crate::sync_storage::InboundApplyOutcome::Applied)
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        _local_device_id: &str,
        policy_identity: Option<crate::sync_storage::PolicyIdentity>,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String> {
        if policy_identity.is_some() {
            return Err("policy_bound_apply_unsupported".into());
        }
        self.apply_authenticated_entity_with_id(entity, authenticated_peer_id, message_id)
            .await
    }
}
