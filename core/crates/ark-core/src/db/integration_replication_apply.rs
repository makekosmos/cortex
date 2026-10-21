#[cfg(test)]
type SignedIntegrationApplyResult = Result<(), crate::integration_replication::SignedSyncError>;

#[cfg(test)]
pub(in crate::db) fn apply_signed_integration_changes(
    database: &ark_store::test_support::IntegrationReplicationTestFixture,
    envelope: &crate::integration_replication::SignedSyncEnvelope,
    expected_space_id: &str,
    expected_recipient_node_id: &str,
    authenticated_transport_public_key: Option<&str>,
    reserved_at_ms: u64,
) -> SignedIntegrationApplyResult {
    database.apply_signed_frame(
        envelope,
        expected_space_id,
        expected_recipient_node_id,
        authenticated_transport_public_key,
        reserved_at_ms,
    )
}
