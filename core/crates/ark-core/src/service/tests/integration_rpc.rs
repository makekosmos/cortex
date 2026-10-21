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

    let published = handle_request(
        &state,
        Request::IntegrationPublishCredentialEnvelope {
            envelope: IntegrationCredentialEnvelope {
                integration_id: "refresh-integration".into(),
                recipient_node_id: "refresh-node".into(),
                grant_epoch: 1,
                credential_generation: 1,
                refresh_fencing_token: 1,
                key_id: crate::integration_replication::encryption_key_id(
                    "encryption-refresh-node",
                ),
                algorithm: "opaque-test".into(),
                nonce: "nonce-refresh".into(),
                ciphertext: "ciphertext-refresh".into(),
                authenticated_metadata: None,
                issuer_node_id: "refresh-node".into(),
                issued_at: "2026-09-06T00:00:00Z".into(),
                revision: 1,
                hlc: "2026-09-06T00:00:00.000Z:000003:refresh-node".into(),
            },
            device_id: "refresh-node".into(),
            now_ms: 1_000,
        },
    )
    .await
    .expect("production writer publishes under the acquired lease");
    assert_eq!(published, json!(true));
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
    let mut persisted = ark_store::test_support::ObjectMutationReplayFixture::inspect(
        &db_path,
        "production-upsert-note",
        "object-upsert-production-replay",
        "object-delete-production-replay",
    )
    .expect("object snapshot and receipts read");
    let mut stale_snapshot = persisted.snapshot.clone();
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
    let first_revision = persisted
        .snapshot
        .revision
        .clone()
        .expect("revision exists");
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

    persisted = ark_store::test_support::ObjectMutationReplayFixture::inspect(
        &db_path,
        "production-upsert-note",
        "object-upsert-production-replay",
        "object-delete-production-replay",
    )
    .expect("persisted object state reads");
    assert_eq!(persisted.upsert_receipts, 1);
    assert_eq!(
        persisted.snapshot.revision.as_deref(),
        Some(first_revision.as_str())
    );
    assert_eq!(
        service
            .call("get_object", json!({ "id": "production-upsert-note" }))
            .await
            .expect("object remains readable")["title"],
        "Note"
    );

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
    let after_first_delete = ark_store::test_support::ObjectMutationReplayFixture::inspect(
        &db_path,
        "production-upsert-note",
        "object-upsert-production-replay",
        "object-delete-production-replay",
    )
    .expect("first deletion state reads");
    let deleted_revision = after_first_delete
        .snapshot
        .revision
        .clone()
        .expect("delete revision exists");
    service
        .call("delete_object", delete("delete-device"))
        .await
        .expect("retry replays the delete receipt");
    let conflict = service
        .call("delete_object", delete("another-device"))
        .await
        .expect_err("operation id reuse with another payload is rejected");
    assert!(conflict.to_string().contains("different command payload"));

    persisted = ark_store::test_support::ObjectMutationReplayFixture::inspect(
        &db_path,
        "production-upsert-note",
        "object-upsert-production-replay",
        "object-delete-production-replay",
    )
    .expect("persisted deletion state reads");
    assert_eq!(persisted.delete_receipts, 1);
    assert_eq!(
        persisted.snapshot.revision.as_deref(),
        Some(deleted_revision.as_str())
    );
    assert_eq!(
        persisted.tombstone_id.as_deref(),
        Some("production-upsert-note")
    );

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
