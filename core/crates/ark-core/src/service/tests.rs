// Service-level tests — the former `main/tests.rs` suite, now driving
// `handle_request` against a per-test `ServiceState` (no process-wide
// statics, so tests run in parallel without mutexes).

use super::runtime::{get_shared_conn, with_conn, with_write_tx};
use super::*;

/// Fresh per-test service state — replaces the old global `DB`/`SYNC`
/// statics and the `TEST_DB_MUTEX` serialization they forced.
pub(super) fn test_state() -> Arc<ServiceState> {
    Arc::new(ServiceState::new())
}

mod legacy_writes;
mod local_writes;
mod object_revision_compat;
mod object_validation;
mod object_write_snapshot;
mod request_config;
/// KOS-51: atomic ARK snapshot restore RPC (list/validate/restore).
mod snapshot_restore;
mod sync_lifecycle;

/// Выставляет capturing-транспорт и выставляет SyncRuntime в `state.sync`.
/// Возвращает буфер перехваченных сообщений.
async fn setup_sync_with_capturing_transport(
    state: &Arc<ServiceState>,
) -> Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>> {
    use crate::relay_sync::{RelaySync, RelaySyncConfig};
    use crate::sync_server::StorageBackend;

    let captured: Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>> =
        Arc::new(TokioMutex::new(Vec::new()));
    let transport = Arc::new(legacy_writes::CapturingTransport {
        sent: captured.clone(),
    });

    let backend = Arc::new(crate::db::SqliteStorageBackend::new(
        get_shared_conn(state).unwrap(),
    ));
    backend.set_device_id("test-device").unwrap();

    let relay = RelaySync::with_transport(
        backend.clone() as Arc<dyn StorageBackend>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            auth_secret: None,
        },
        transport as Arc<dyn crate::sync_transport::SyncTransport>,
    );
    relay.start().await.unwrap();

    let server = Arc::new(crate::sync_server::SyncServer::new(
        backend.clone() as Arc<dyn StorageBackend>
    ));

    let runtime = SyncRuntime {
        server,
        storage: backend,
        clients: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
        relay: Some(relay),
        transport_choice: Some(TransportChoice::Relay),
        start_params: SyncStartParams {
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            port: None,
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: false,
            iroh_peer_ticket: None,
            discovery_enabled: true,
            bind: SyncBind::AllInterfaces,
        },
        iroh_our_ticket: None,
        beacon: Arc::new(crate::beacon::BroadcastDiscovery::new()),
        space_id: "test-space".to_string(),
        device_id: "test-device".to_string(),
        device_name: "Test Device".to_string(),
        auth_secret: None,
        own_addresses: Arc::new(TokioMutex::new(Vec::new())),
    };
    *state.sync.lock().await = Some(Arc::new(runtime));

    captured
}

fn canonical_task_object(type_version: Option<&str>, props_json: Value) -> ArkObjectWrite {
    ArkObjectWrite {
        id: "canonical-task-ingress".to_string(),
        type_id: "com.kosmos.task".to_string(),
        type_version: type_version.map(str::to_owned),
        title: "Canonical task".to_string(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello"}]}]}),
        props_json,
        created_at: "2026-08-11T00:00:00.000Z".to_string(),
        updated_at: "2026-08-11T00:00:00.000Z".to_string(),
        deleted_at: None,
    }
}

fn canonical_task_props() -> Value {
    json!({"status":"todo","priority":"medium","scheduledAt":null,"dueAt":null,"reminderAt":null,"completedAt":null,"canceledAt":null,"recurrence":null,"checklist":[],"extensions":{"vendor":{"opaque":true}}})
}

#[test]
fn dotted_type_rpc_hits_real_handler_and_omitted_upsert_resolves_current() {
    let state = test_state();
    let path = std::env::temp_dir().join(format!("ark-phase2-rpc-{}.db", std::process::id()));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        handle_request(&state, Request::Init { db_path: path.to_string_lossy().into_owned() }).await.unwrap();
        let type_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object_type",
            "object_type": {"id":"rpc-phase2", "name":"RPC", "schemaJson":"{}", "uiSchemaJson":"{}", "createdAt":"c", "updatedAt":"u", "systemLocked":false}
        })).unwrap();
        handle_request(&state, type_request).await.unwrap();
        let object_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object",
            "object": {"id":"rpc-object", "typeId":"rpc-phase2", "title":"x", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
        })).unwrap();
        handle_request(&state, object_request).await.unwrap();
        let stored = handle_request(&state, Request::GetObject { id: "rpc-object".into() }).await.unwrap();
        assert!(stored["typeVersion"]
            .as_str()
            .is_some_and(|version| version.starts_with("0.0.0+legacy.")));
        let mut event_rx = crate::events::subscribe();
        let before = with_conn(&state, |conn| {
            Ok((
                conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                db::get_sync_kv(conn, "lan_sync.version_vector")?,
            ))
        }).unwrap();
        let unknown_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object",
            "object": {"id":"rpc-unknown", "typeId":"rpc-phase2", "typeVersion":"9.9.9", "title":"unknown", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
        })).unwrap();
        assert!(handle_request(&state, unknown_request).await.is_err());
        let after = with_conn(&state, |conn| {
            Ok((
                conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                db::get_sync_kv(conn, "lan_sync.version_vector")?,
            ))
        }).unwrap();
        assert_eq!(before, after);
        assert!(!matches!(event_rx.try_recv(), Ok(event) if event["event"] == "object_upserted"));
        let result = handle_request(&state, Request::TypesGet {
            type_id: "rpc-phase2".into(),
            version: None,
        })
        .await
        .unwrap();
        assert!(result["summary"].is_object());
        assert!(result["definition"].is_object());
        let alias = handle_request(&state, Request::TypesResolveAlias { alias: "not-an-alias".into() }).await.unwrap();
        assert!(alias.is_null());
        let versions = handle_request(&state, Request::TypesListVersions { type_id: "missing".into() }).await.unwrap();
        assert_eq!(versions, json!([]));
    });
    std::fs::remove_file(path).ok();
}

#[test]
fn dotted_type_rpc_handlers_cover_aliases_versions_nulls_and_ordering() {
    let state = test_state();
    let path =
        std::env::temp_dir().join(format!("ark-phase2-rpc-matrix-{}.db", std::process::id()));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        handle_request(
            &state,
            Request::Init {
                db_path: path.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
        for id in ["rpc-z", "rpc-a"] {
            handle_request(
                &state,
                Request::UpsertObjectType {
                    object_type: ObjectType {
                        id: id.into(),
                        name: id.into(),
                        schema_json: "{}".into(),
                        ui_schema_json: "{}".into(),
                        created_at: "c".into(),
                        updated_at: "u".into(),
                        system_locked: false,
                    },
                    device_id: None,
                },
            )
            .await
            .unwrap();
        }
        with_conn(&state, |conn| {
            crate::type_registry::register_alias(
                conn,
                &crate::type_registry::AliasRecord {
                    alias: "rpc.alias".into(),
                    canonical_type_id: "rpc-a".into(),
                    created_at: "a".into(),
                },
            )
        })
        .unwrap();
        let list = handle_request(&state, Request::TypesList).await.unwrap();
        let list_ids = list
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["typeId"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(list_ids.windows(2).all(|w| w[0] <= w[1]));
        let alias_get = handle_request(
            &state,
            Request::TypesGet {
                type_id: "rpc.alias".into(),
                version: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(alias_get["summary"]["typeId"], "rpc-a");
        let alias_versions = handle_request(
            &state,
            Request::TypesListVersions {
                type_id: "rpc.alias".into(),
            },
        )
        .await
        .unwrap();
        assert!(!alias_versions.as_array().unwrap().is_empty());
        assert!(handle_request(
            &state,
            Request::TypesGet {
                type_id: "unknown".into(),
                version: None
            }
        )
        .await
        .unwrap()
        .is_null());
        assert_eq!(
            handle_request(
                &state,
                Request::TypesResolveAlias {
                    alias: "unknown".into()
                }
            )
            .await
            .unwrap(),
            Value::Null
        );
        assert_eq!(
            handle_request(
                &state,
                Request::TypesListVersions {
                    type_id: "unknown".into()
                }
            )
            .await
            .unwrap(),
            json!([])
        );
        let exact_keys = alias_get
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            exact_keys.contains(&"summary".into()) && exact_keys.contains(&"definition".into())
        );
    });
    std::fs::remove_file(path).ok();
}

#[test]
fn dotted_type_operations_are_exact_and_underscore_aliases_rejected() {
    for operation in [
        "types.list",
        "types.get",
        "types.listVersions",
        "types.resolveAlias",
    ] {
        let mut value = json!({"operation": operation});
        if operation == "types.get" || operation == "types.listVersions" {
            value["typeId"] = json!("x");
        }
        if operation == "types.resolveAlias" {
            value["alias"] = json!("x");
        }
        assert!(
            serde_json::from_value::<Request>(value).is_ok(),
            "{operation}"
        );
    }
    for operation in [
        "types_list",
        "types_get",
        "types_list_versions",
        "types_resolve_alias",
    ] {
        assert!(
            serde_json::from_value::<Request>(json!({"operation": operation})).is_err(),
            "{operation}"
        );
    }
}

#[tokio::test]
async fn canonical_upsert_object_rpc_persists_registered_identity() {
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
    handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), canonical_task_props()),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await
    .unwrap();
    let object = with_conn(&state, |conn| {
        db::get_object(conn, "canonical-task-ingress")
    })
    .unwrap()
    .unwrap();
    assert_eq!(object.type_id, "com.kosmos.task");
    assert_eq!(object.type_version, "1.0.0");
    assert_eq!(object.props_json["status"], "todo");
}

#[tokio::test]
async fn canonical_upsert_object_rpc_rejects_omitted_version_without_side_effects() {
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
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(None, canonical_task_props()),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:canonical_version_required"
    );
    let object_count = with_conn(&state, |conn| {
        conn.query_row("SELECT COUNT(*) FROM objects", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(object_count, 0);
}

#[tokio::test]
async fn canonical_upsert_object_rpc_rejects_invalid_payload_without_sync_mutation() {
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

    let mut props = canonical_task_props();
    props["unexpected"] = json!(true);
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), props),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:canonical_field:/unexpected"
    );
    let (objects, versions) = with_conn(&state, |conn| {
        let objects = conn
            .query_row("SELECT COUNT(*) FROM objects", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())?;
        let versions = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_kv WHERE key = 'lan_sync.version_vector'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((objects, versions))
    })
    .unwrap();
    assert_eq!((objects, versions), (0, 0));
}

#[tokio::test]
async fn canonical_upsert_object_rpc_rejects_legacy_alias_as_new_write() {
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
    let mut object = canonical_task_object(Some("1.0.0"), canonical_task_props());
    object.type_id = "task_obj".to_string();
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: None,
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:legacy_alias_new_write"
    );
}

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
        Request::IntegrationAcquireRefreshLease(AcquireRefreshLeaseParams {
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
