pub use ark_application::replication_storage::{
    AuthenticatedEntityBackend, PeerIdentityBackend, RegistryPageApply,
};
pub use ark_application::sync_storage::SyncRuntimeStorage;

#[cfg(test)]
mod tests {
    use crate::types::VersionVector;
    use ark_application::sync_storage::SyncReadClass;

    #[test]
    fn usage_only_peer_vector_is_still_backfill() {
        let mut vector = VersionVector::new();
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::Backfill
        );
        vector.insert("@usage:phone".into(), "23".into());
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::Backfill
        );
        vector.insert("object-1".into(), "2026-01-01:000001:desktop".into());
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::SyncLive
        );
    }

    #[test]
    fn unfinished_sync_stays_backfill_even_with_a_partial_vector() {
        assert_eq!(
            SyncReadClass::for_sync_state(false),
            SyncReadClass::Backfill
        );
        assert_eq!(SyncReadClass::for_sync_state(true), SyncReadClass::SyncLive);
    }
}
