use super::*;

#[tokio::test]
async fn integration_rpc_rejects_revocation_without_local_authorization() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let request: Request = serde_json::from_value(json!({
        "operation": "integration.persist_node_authorization",
        "authorization_operation": "revoke",
        "node": {
            "node_id": "recipient",
            "key_fingerprint": "fingerprint",
            "signing_public_key": "signing",
            "encryption_public_key": "encryption",
            "grant_epoch": 2,
            "status": "revoked",
            "authorized_at": "2026-09-05T00:00:00Z",
            "revoked_at": "2026-09-05T00:01:00Z",
            "revocation_epoch": 2,
            "revision": 2,
            "hlc": "2026-09-05T00:00:00.000Z:000001:recipient"
        },
        "device_id": "rpc"
    }))
    .unwrap();

    let error = handle_request(&state, request).await.unwrap_err();
    assert_eq!(error, "node is not authorized");
}

#[tokio::test]
async fn integration_rpc_rejects_stale_refresh_lease_fence() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let node = serde_json::from_value(json!({
        "operation": "integration.persist_node_authorization",
        "authorization_operation": "authorize",
        "node": {
            "node_id": "refresh-node",
            "key_fingerprint": "fingerprint-refresh-node",
            "signing_public_key": "signing-refresh-node",
            "encryption_public_key": "encryption-refresh-node",
            "transport_public_key": null,
            "grant_epoch": 1,
            "status": "active",
            "authorized_at": "2026-09-06T00:00:00Z",
            "revoked_at": null,
            "revocation_epoch": null,
            "revision": 1,
            "hlc": "2026-09-06T00:00:00.000Z:000001:refresh-node"
        },
        "grant": {
            "integration_id": "refresh-integration",
            "node_id": "refresh-node",
            "node_encryption_key": "encryption-refresh-node",
            "grant_epoch": 1,
            "status": "active",
            "authorized_at": "2026-09-06T00:00:00Z",
            "revoked_at": null,
            "revision": 1,
            "hlc": "2026-09-06T00:00:00.000Z:000002:refresh-node"
        },
        "device_id": "refresh-node"
    }))
    .unwrap();
    handle_request(&state, node).await.unwrap();

    let acquire = |expected_fencing_token| {
        Request::IntegrationAcquireRefreshLease(RefreshLeaseAcquireParams {
            integration_id: "refresh-integration".into(),
            holder_node_id: "refresh-node".into(),
            credential_generation: 1,
            now_ms: 1_000,
            ttl_ms: 100,
            expected_fencing_token,
            device_id: "refresh-node".into(),
        })
    };
    let lease = handle_request(&state, acquire(0)).await.unwrap();
    assert_eq!(lease["fencing_token"], 1);
    let error = handle_request(&state, acquire(0)).await.unwrap_err();
    assert!(error.contains("expected_fencing_token"));
}

#[tokio::test]
async fn integration_issuer_key_lookup_rejects_wrong_space() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    handle_request(
        &state,
        Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();
    setup_sync_with_capturing_transport(&state).await;

    let error = handle_request(
        &state,
        Request::IntegrationLookupIssuerEncryptionKey {
            space_id: "wrong-space".into(),
            integration_id: "integration-a".into(),
            recipient_node_id: "recipient".into(),
            issuer_node_id: "issuer".into(),
            credential_generation: 1,
            expected_issuer_key_id: "sha256:issuer".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error, "integration lookup requested for the wrong space");
    let prepublish_error = handle_request(
        &state,
        Request::IntegrationLookupIssuerEncryptionKeyForPublish {
            space_id: "wrong-space".into(),
            integration_id: "integration-a".into(),
            recipient_node_id: "recipient".into(),
            issuer_node_id: "issuer".into(),
            expected_issuer_key_id: "sha256:issuer".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        prepublish_error,
        "integration lookup requested for the wrong space"
    );
    // Deterministic teardown: StopSync stops the relay transport and joins
    // its background tasks so the shared conn closes before the fixture's
    // TempDir is removed (KOS-270).
    handle_request(&state, Request::StopSync).await.unwrap();
}

#[test]
fn integration_issuer_key_lookup_uses_the_versioned_wire_shape() {
    let request: Request = serde_json::from_value(json!({
        "operation": "integration.lookup_issuer_encryption_key",
        "space_id": "space-a",
        "integration_id": "integration-a",
        "recipient_node_id": "node-a",
        "issuer_node_id": "node-b",
        "credential_generation": 3,
        "expected_issuer_key_id": "sha256:issuer"
    }))
    .unwrap();
    assert!(matches!(
        request,
        Request::IntegrationLookupIssuerEncryptionKey {
            credential_generation: 3,
            ..
        }
    ));
    assert!(serde_json::from_value::<Request>(json!({
        "operation": "integration.lookup_issuer_encryption_key",
        "spaceId": "space-a"
    }))
    .is_err());
}

/// A handler panic leaves its result unknown and stops the service worker.
#[tokio::test]
async fn panicking_request_stops_service_until_explicit_reopen() {
    let dir = crate::test_support::temp_dir();
    let db_path = dir.path().join("ark.db");
    let service = ArkService::open(db_path.to_str().unwrap())
        .await
        .expect("service opens");

    let error = service
        .call("test.panic", json!({}))
        .await
        .expect_err("panicking request must stop the worker");
    assert!(
        matches!(
            error,
            ArkServiceError::Unavailable(ref message)
                if message.contains("outcome unknown") && message.contains("handler panic")
        ),
        "expected an unavailable error with unknown outcome, got {error:?}"
    );

    let unavailable = service
        .call("load_all", json!({}))
        .await
        .expect_err("the failed service must reject later requests");
    assert!(
        matches!(unavailable, ArkServiceError::Unavailable(_)),
        "expected the failed worker to be unavailable, got {unavailable:?}"
    );
    assert_eq!(service.panic_count(), 1);

    service
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await
        .expect("failed worker shuts down");
    let restarted = ArkService::open(db_path.to_str().unwrap())
        .await
        .expect("explicit service reopen");
    let data = restarted
        .call("load_all", json!({}))
        .await
        .expect("new service serves after bootstrap");
    assert!(data.is_object(), "load_all returned {data:?}");
    restarted
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await
        .expect("reopened service shuts down");
}

#[tokio::test]
async fn oversized_request_is_rejected_before_queue_and_service_keeps_serving() {
    let dir = crate::test_support::temp_dir();
    let db_path = dir.path().join("ark.db");
    let service = ArkService::open(db_path.to_string_lossy().to_string())
        .await
        .expect("service opens");

    let result = service
        .call(
            "test.oversized",
            json!({ "payload": "x".repeat(9 * 1024 * 1024) }),
        )
        .await;
    assert!(matches!(result, Err(ArkServiceError::Busy(_))));

    assert!(service.call("load_all", json!({})).await.is_ok());
    service
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await
        .expect("service shuts down before the temporary database is removed");
}

#[tokio::test]
async fn production_writer_object_mutations_replay_receipts_without_new_revisions() {
    let dir = crate::test_support::temp_dir();
    let db_path = dir.path().join("ark.db");
    let service = ArkService::open(db_path.to_str().unwrap())
        .await
        .expect("service opens");
    let request = |title: &str| {
        json!({
            "operationId": "object-upsert-production-replay",
            "deviceId": "ignored-for-principal",
            "object": {
                "id": "production-upsert-note",
                "typeId": "com.kosmos.note",
                "typeVersion": "1.0.0",
                "title": title,
                "contentJson": {"type":"doc","content":[]},
                "propsJson": {"description":null,"extensions":{}},
                "createdAt": "2026-10-19T00:00:00.000Z",
                "updatedAt": "2026-10-19T00:00:00.000Z",
                "deletedAt": null
            }
        })
    };

    assert_eq!(
        service
            .call("upsert_object", request("Note"))
            .await
            .expect("first upsert commits"),
        json!(true)
    );
    let mut stale_snapshot = {
        let connection = rusqlite::Connection::open(&db_path).expect("database opens");
        ark_store::object_write::read_object_write_snapshot(&connection, "production-upsert-note")
            .expect("object snapshot reads")
    };
    stale_snapshot.revision = Some("stale-revision".into());
    let stale_delete = service
        .call(
            "delete_object",
            json!({
                "id": "production-upsert-note",
                "expectedSnapshot": stale_snapshot,
                "device_id": "stale-delete-device"
            }),
        )
        .await
        .expect_err("stale snapshot is rejected");
    assert!(stale_delete.to_string().contains("stale_snapshot"));
    assert_eq!(
        service
            .call("get_object", json!({ "id": "production-upsert-note" }))
            .await
            .expect("stale delete leaves the object readable")["title"],
        "Note"
    );
    let first_revision = {
        let connection = rusqlite::Connection::open(&db_path).expect("database opens");
        connection
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id='production-upsert-note'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("revision exists")
    };
    assert_eq!(
        service
            .call("upsert_object", request("Note"))
            .await
            .expect("retry replays"),
        json!(true)
    );
    let conflict = service
        .call("upsert_object", request("Different payload"))
        .await
        .expect_err("operation id reuse with another payload is rejected");
    assert!(conflict.to_string().contains("different command payload"));

    let connection = rusqlite::Connection::open(&db_path).expect("database opens");
    let (receipts, revision, title): (i64, String, String) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM ark_operation_receipts
                     WHERE principal='engine' AND operation_id='object-upsert-production-replay'),
                    object_sync_versions.hlc, objects.title
             FROM object_sync_versions JOIN objects ON objects.id=object_sync_versions.object_id
             WHERE object_sync_versions.object_id='production-upsert-note'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("receipt, revision, and object read");
    assert_eq!(receipts, 1);
    assert_eq!(revision, first_revision);
    assert_eq!(title, "Note");

    let delete = |device_id: &str| {
        json!({
            "id": "production-upsert-note",
            "operationId": "object-delete-production-replay",
            "device_id": device_id,
        })
    };
    service
        .call("delete_object", delete("delete-device"))
        .await
        .expect("first delete commits");
    let deleted_revision = {
        let connection = rusqlite::Connection::open(&db_path).expect("database opens");
        connection
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id='production-upsert-note'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("delete revision exists")
    };
    service
        .call("delete_object", delete("delete-device"))
        .await
        .expect("retry replays the delete receipt");
    let conflict = service
        .call("delete_object", delete("another-device"))
        .await
        .expect_err("operation id reuse with another payload is rejected");
    assert!(conflict.to_string().contains("different command payload"));

    let connection = rusqlite::Connection::open(&db_path).expect("database opens");
    let (delete_receipts, final_revision, tombstone): (i64, String, Option<String>) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM ark_operation_receipts
                     WHERE principal='engine' AND operation_id='object-delete-production-replay'),
                    object_sync_versions.hlc,
                    (SELECT id FROM sync_tombstones WHERE id='production-upsert-note')
             FROM object_sync_versions WHERE object_id='production-upsert-note'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("delete receipt, revision, and tombstone read");
    assert_eq!(delete_receipts, 1);
    assert_eq!(final_revision, deleted_revision);
    assert_eq!(tombstone.as_deref(), Some("production-upsert-note"));

    service
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await
        .expect("service shuts down");
}

#[test]
fn integration_issuer_key_lookup_for_publish_uses_the_exact_wire_shape() {
    let request: Request = serde_json::from_value(json!({
        "operation": "integration.lookup_issuer_encryption_key_for_publish",
        "space_id": "space-a",
        "integration_id": "integration-a",
        "recipient_node_id": "node-a",
        "issuer_node_id": "node-b",
        "expected_issuer_key_id": "sha256:issuer"
    }))
    .unwrap();
    assert!(matches!(
        request,
        Request::IntegrationLookupIssuerEncryptionKeyForPublish { .. }
    ));
}
