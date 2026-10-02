fn setup_refresh_authority(conn: &Connection) {
    upsert_integration_configuration(conn, &integration_config(serde_json::json!({})), "node-a")
        .unwrap();
    upsert_authorized_node(conn, &integration_node(), "node-a").unwrap();
    upsert_integration_node_grant(conn, &integration_grant(), "node-a").unwrap();
}

#[test]
fn publication_requires_the_current_refresh_fence() {
    let conn = integration_test_db();
    setup_refresh_authority(&conn);
    let lease = acquire_integration_test_lease(&conn, "node-a");
    let mut envelope = integration_envelope(lease.credential_generation);
    envelope.refresh_fencing_token = lease.fencing_token.saturating_add(1);
    assert!(publish_integration_credential_envelope(&conn, &envelope, "node-a", 15).is_err());
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM integration_credential_envelopes",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);

    envelope.refresh_fencing_token = lease.fencing_token;
    publish_integration_credential_envelope(&conn, &envelope, "node-a", 15).unwrap();
    assert_eq!(
        load_integration_credential_envelope(&conn, "integration-a", "node-a", 3).unwrap(),
        Some(envelope)
    );
    assert!(
        load_latest_integration_credential_envelope(&conn, "integration-a", "node-a")
            .unwrap()
            .is_some()
    );
    assert!(
        load_latest_integration_credential_envelope(&conn, "integration-a", "node-b")
            .unwrap()
            .is_none()
    );

    assert!(try_acquire_integration_refresh_lease(
        &conn,
        &RefreshLeaseAcquireParams {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 20,
            ttl_ms: 10,
            expected_fencing_token: lease.fencing_token,
            device_id: "node-a".into(),
        },
    )
    .is_err());
    assert_eq!(
        load_integration_refresh_lease(&conn, "integration-a")
            .unwrap()
            .unwrap(),
        lease
    );
}

#[test]
fn local_refresh_writes_cannot_impersonate_another_node() {
    let conn = integration_test_db();
    setup_refresh_authority(&conn);
    assert!(try_acquire_integration_refresh_lease(
        &conn,
        &RefreshLeaseAcquireParams {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-b".into(),
        },
    )
    .is_err());

    let lease = acquire_integration_test_lease(&conn, "node-a");
    let mut envelope = integration_envelope(lease.credential_generation);
    envelope.refresh_fencing_token = lease.fencing_token;
    assert!(publish_integration_credential_envelope(&conn, &envelope, "node-b", 15,).is_err());
}

#[test]
fn rotated_recipient_rejects_stale_key_publication_without_vector_change() {
    let conn = integration_test_db();
    setup_refresh_authority(&conn);

    let mut rotated_node = integration_node();
    rotated_node.key_fingerprint = "fp-a-rotated".into();
    rotated_node.encryption_public_key = "enc-a-rotated".into();
    rotated_node.grant_epoch += 1;
    rotated_node.revision += 1;
    rotated_node.hlc = "2026-08-30T00:00:00.000Z:000002:node-a".into();
    let mut rotated_grant = integration_grant();
    rotated_grant.node_encryption_key = rotated_node.encryption_public_key.clone();
    rotated_grant.grant_epoch = rotated_node.grant_epoch;
    rotated_grant.revision += 1;
    rotated_grant.hlc = "2026-08-30T00:00:00.000Z:000003:node-a".into();
    crate::integration_replication::persist_node_authorization(
        &conn,
        crate::integration_replication::NodeAuthorizationOperation::Rotate,
        &rotated_node,
        Some(&rotated_grant),
        "node-a",
    )
    .unwrap();

    let lease = acquire_integration_test_lease(&conn, "node-a");
    let mut envelope = integration_envelope(lease.credential_generation);
    envelope.grant_epoch = rotated_node.grant_epoch;
    envelope.refresh_fencing_token = lease.fencing_token;
    let stale_key_id = envelope.key_id.clone();
    let vector_before = get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap();

    assert!(publish_integration_credential_envelope(&conn, &envelope, "node-a", 15).is_err());
    assert_eq!(
        get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap(),
        vector_before
    );
    assert!(load_integration_credential_envelope(
        &conn,
        "integration-a",
        "node-a",
        lease.credential_generation,
    )
    .unwrap()
    .is_none());

    envelope.key_id =
        crate::integration_replication::encryption_key_id(&rotated_node.encryption_public_key);
    assert_ne!(envelope.key_id, stale_key_id);
    publish_integration_credential_envelope(&conn, &envelope, "node-a", 15).unwrap();
}

#[test]
fn expired_unpublished_generation_retries_with_a_higher_fence() {
    let conn = integration_test_db();
    setup_refresh_authority(&conn);
    let first = acquire_integration_test_lease(&conn, "node-a");

    let retry = try_acquire_integration_refresh_lease(
        &conn,
        &RefreshLeaseAcquireParams {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: first.credential_generation,
            now_ms: first.expires_at_ms,
            ttl_ms: 10,
            expected_fencing_token: first.fencing_token,
            device_id: "node-a".into(),
        },
    )
    .unwrap();

    assert_eq!(retry.credential_generation, first.credential_generation);
    assert_eq!(retry.fencing_token, first.fencing_token + 1);
}

#[test]
fn concurrent_refresh_acquire_has_one_fence_winner() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("integration-refresh.sqlite");
    let setup = open_db(path.to_str().unwrap()).unwrap();
    setup.execute_batch(crate::schema::CREATE_TABLES).unwrap();
    setup_refresh_authority(&setup);
    drop(setup);

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                let conn = open_db(path.to_str().unwrap()).unwrap();
                barrier.wait();
                try_acquire_integration_refresh_lease(
                    &conn,
                    &RefreshLeaseAcquireParams {
                        integration_id: "integration-a".into(),
                        holder_node_id: "node-a".into(),
                        credential_generation: 3,
                        now_ms: 10,
                        ttl_ms: 10,
                        expected_fencing_token: 0,
                        device_id: "node-a".into(),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(
        results
            .iter()
            .find_map(|result| result.as_ref().ok())
            .unwrap()
            .fencing_token,
        1
    );
}
