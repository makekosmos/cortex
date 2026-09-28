use super::replication_requires_authority;

#[test]
fn every_replication_operation_requires_desktop_authority() {
    for operation in [
        "replication_acquire_refresh_lease",
        "replication_send_signed_sync",
        "replication_publish_credential_envelope_v2",
    ] {
        assert!(replication_requires_authority(operation));
    }
    assert!(!replication_requires_authority("list"));
}
