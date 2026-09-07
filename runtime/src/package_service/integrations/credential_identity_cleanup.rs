use super::*;

/// Remove a test-owned host identity from the OS keyring after an isolated
/// replication fixture. Production callers never need to delete identities.
#[cfg(feature = "package-worker-fixture")]
pub fn clear_identity(node_id: &str) -> Result<(), CredentialEnvelopeError> {
    validate_id(node_id)?;
    clear_package_integration_secret(IDENTITY_PACKAGE, IDENTITY_VERSION, &identity_setting(node_id))
        .map_err(CredentialEnvelopeError::from)
}

#[cfg(not(feature = "package-worker-fixture"))]
pub fn clear_identity(_node_id: &str) -> Result<(), CredentialEnvelopeError> {
    Err(CredentialEnvelopeError::InvalidEnvelope)
}
