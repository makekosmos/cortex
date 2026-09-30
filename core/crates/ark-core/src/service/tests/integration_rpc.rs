use super::*;

#[tokio::test]
async fn integration_rpc_rejects_revocation_without_local_authorization() {
    let state = test_state();
    let dir = tempfile::tempdir().unwrap();
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
    let state = test_state();
    let dir = tempfile::tempdir().unwrap();
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
    let state = test_state();
    let dir = tempfile::tempdir().unwrap();
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
    *state.sync.lock().await = None;
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

/// Fault isolation: a panicking handler returns an error, the worker catches
/// it, reopens the database, and keeps serving subsequent requests.
#[tokio::test]
async fn panicking_request_returns_error_and_service_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    let service = ArkService::open(db_path.to_str().unwrap())
        .await
        .expect("service opens");

    let error = service
        .call("test.panic", json!({}))
        .await
        .expect_err("panicking op must surface an error, not kill the worker");
    assert!(
        matches!(error, ArkServiceError::Request(ref message) if message.contains("panicked")),
        "expected a request error describing the panic, got {error:?}"
    );

    let data = service
        .call("load_all", json!({}))
        .await
        .expect("service must recover after a handler panic");
    assert!(data.is_object(), "load_all returned {data:?}");
    assert_eq!(service.panic_count(), 1);
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
