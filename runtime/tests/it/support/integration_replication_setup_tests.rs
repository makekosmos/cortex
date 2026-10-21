use super::*;

#[test]
fn fixture_seeds_both_isolated_databases() {
    let _identities = HOST_IDENTITIES.blocking_lock();
    let setup = new_setup_named(INTEGRATION_ID).unwrap();
    for path in [&setup.origin_db, &setup.recipient_db] {
        let conn = Connection::open(path).unwrap();
        assert!(
            ark_store::integration_replication::load_integration_configuration(
                &conn,
                INTEGRATION_ID
            )
            .unwrap()
            .is_some()
        );
        assert!(ark_store::integration_replication::load_authorized_node(
            &conn,
            &setup.origin.node_id
        )
        .unwrap()
        .is_some());
        assert!(
            ark_store::integration_replication::load_integration_node_grant(
                &conn,
                INTEGRATION_ID,
                &setup.recipient.node_id,
            )
            .unwrap()
            .is_some()
        );
    }
    let conn = Connection::open(&setup.origin_db).unwrap();
    assert!(ark_store::integration_replication::load_authorized_node(
        &conn,
        &setup.foreign.node_id
    )
    .unwrap()
    .is_some());
    let signature = setup.origin.signing_key.sign(b"integration-replication");
    setup
        .origin
        .signing_key
        .verifying_key()
        .verify_strict(b"integration-replication", &signature)
        .unwrap();
    let mut reader =
        ark_store::read_snapshot::SqliteReader::open_read_only(setup.origin_db.to_str().unwrap())
            .unwrap();
    let prepared = reader
        .with_read_snapshot(|capabilities| {
            ark_core::integration_replication::prepare_signed_sync(
                &capabilities.integration_replication(),
                "synthetic-space",
                &setup.origin.node_id,
                INTEGRATION_ID,
                &setup.recipient.node_id,
                "synthetic-message",
            )
        })
        .unwrap();
    assert!(!prepared.envelope.payload.is_empty());
}
