use super::*;

#[async_trait::async_trait]
impl SyncEntityReadPort for TestStorage {
    async fn load_entities(
        &self,
        _vector: &ark_core::types::VersionVector,
    ) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }

    async fn load_entities_page(
        &self,
        _vector: &ark_core::types::VersionVector,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }
    async fn load_entities_page_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
        vector: &ark_core::VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.inner
            .load_entities_page_for_peer(peer, class, vector, offset, limit)
            .await
    }
    async fn load_policy_bound_entities_page_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
        vector: &ark_core::VersionVector,
        offset: usize,
        limit: usize,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundSnapshotPage,
        ark_application::sync_storage::PolicyBoundSnapshotReadError,
    > {
        self.inner
            .load_policy_bound_entities_page_for_peer(
                peer,
                class,
                vector,
                offset,
                limit,
                expected_identity,
            )
            .await
    }
    async fn usage_complete_through(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.inner.usage_complete_through().await
    }
    async fn usage_complete_through_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.inner
            .usage_complete_through_for_peer(peer, class)
            .await
    }
    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        self.inner.filter_outgoing_entity(entity)
    }
    async fn filter_outgoing_entity_for_peer(
        &self,
        peer: &str,
        entity: &SyncEntity,
    ) -> Result<Option<SyncEntity>, String> {
        self.inner
            .filter_outgoing_entity_for_peer(peer, entity)
            .await
    }
}
#[async_trait::async_trait]
impl SyncEntityMutationPort for TestStorage {
    async fn apply_entity(&self, _entity: &SyncEntity) -> Result<(), String> {
        Ok(())
    }
}
