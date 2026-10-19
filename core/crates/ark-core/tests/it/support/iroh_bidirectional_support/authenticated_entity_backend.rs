use super::*;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for TestStorage {
    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}
