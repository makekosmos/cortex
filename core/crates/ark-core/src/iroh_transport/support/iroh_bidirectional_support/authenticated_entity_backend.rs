use super::*;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for TestStorage {
    async fn receive_authenticated_entity_with_id(
        &self,
        _: &SyncEntity,
        _: &str,
        _: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        _: &SyncEntity,
        _: &str,
        _: &str,
        _: &str,
        policy: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<bool, String> {
        if policy.is_some() {
            return Err("policy_bound_receipt_unsupported".into());
        }
        Ok(false)
    }

    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        peer: &str,
        _: &str,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        self.apply_authenticated_entity(entity, peer).await?;
        Ok(ark_application::sync_storage::InboundApplyOutcome::Applied)
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        peer: &str,
        message_id: &str,
        _: &str,
        policy: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        if policy.is_some() {
            return Err("policy_bound_apply_unsupported".into());
        }
        self.apply_authenticated_entity_with_id(entity, peer, message_id)
            .await
    }
}
