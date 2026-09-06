#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ark_binary_declares_windows_gui_subsystem_feature() {
        let source = include_str!("../main.rs");
        let cargo = include_str!("../../Cargo.toml");

        assert!(cargo.contains("windows-gui-subsystem = []"));
        assert!(source.contains("all(windows, feature = \"windows-gui-subsystem\")"));
        assert!(source.contains("windows_subsystem = \"windows\""));
    }

    static TEST_DB_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static TEST_EVENT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[path = "request_config.rs"]
    mod request_config;
    #[path = "local_writes.rs"]
    mod local_writes;
    #[path = "object_write_snapshot.rs"]
    mod object_write_snapshot;
    #[path = "object_revision_compat.rs"]
    mod object_revision_compat;

    /// Regression for the legacy Todo/Project/Tag write handlers: local writes
    /// must record an HLC in the version vector and deletes must write a tombstone.
    #[path = "legacy_writes.rs"]
    mod legacy_writes;
    /// и выставляет в глобальный SYNC. Возвращает буфер перехваченных сообщений.
    async fn setup_sync_with_capturing_transport(
        shared_conn: Arc<StdMutex<rusqlite::Connection>>,
    ) -> Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>> {
        use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
        use ark_core::sync_server::StorageBackend;

        let captured: Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>> =
            Arc::new(TokioMutex::new(Vec::new()));
        let transport = Arc::new(legacy_writes::CapturingTransport {
            sent: captured.clone(),
        });

        let backend = Arc::new(ark_core::db::SqliteStorageBackend::new(shared_conn));
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
            transport as Arc<dyn ark_core::sync_transport::SyncTransport>,
        );
        relay.start().await.unwrap();

        let server = Arc::new(ark_core::sync_server::SyncServer::new(
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
            },
            iroh_our_ticket: None,
            beacon: Arc::new(ark_core::beacon::BroadcastDiscovery::new()),
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            auth_secret: None,
            own_addresses: Arc::new(TokioMutex::new(Vec::new())),
        };
        *SYNC.lock().await = Some(Arc::new(runtime));

        captured
    }

    #[path = "sync_lifecycle.rs"]
    mod sync_lifecycle;
    #[path = "object_validation.rs"]
    mod object_validation;
    #[test]
    fn dotted_type_rpc_hits_real_handler_and_omitted_upsert_resolves_current() {
        let _guard = TEST_DB_MUTEX.blocking_lock();
        let path = std::env::temp_dir().join(format!("ark-phase2-rpc-{}.db", std::process::id()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            handle_request(Request::Init { db_path: path.to_string_lossy().into_owned() }).await.unwrap();
            let type_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object_type",
                "object_type": {"id":"rpc-phase2", "name":"RPC", "schemaJson":"{}", "uiSchemaJson":"{}", "createdAt":"c", "updatedAt":"u", "systemLocked":false}
            })).unwrap();
            handle_request(type_request).await.unwrap();
            let object_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object",
                "object": {"id":"rpc-object", "typeId":"rpc-phase2", "title":"x", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
            })).unwrap();
            handle_request(object_request).await.unwrap();
            let stored = handle_request(Request::GetObject { id: "rpc-object".into() }).await.unwrap();
            assert!(stored["typeVersion"]
                .as_str()
                .is_some_and(|version| version.starts_with("0.0.0+legacy.")));
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            set_event_sender(event_tx);
            let before = with_conn(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                    db::get_sync_kv(conn, "lan_sync.version_vector")?,
                ))
            }).unwrap();
            let unknown_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object",
                "object": {"id":"rpc-unknown", "typeId":"rpc-phase2", "typeVersion":"9.9.9", "title":"unknown", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
            })).unwrap();
            assert!(handle_request(unknown_request).await.is_err());
            let after = with_conn(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                    db::get_sync_kv(conn, "lan_sync.version_vector")?,
                ))
            }).unwrap();
            assert_eq!(before, after);
            assert!(!matches!(event_rx.try_recv(), Ok(event) if event["event"] == "object_upserted"));
            let result = handle_request(Request::TypesGet {
                type_id: "rpc-phase2".into(),
                version: None,
            })
            .await
            .unwrap();
            assert!(result["summary"].is_object());
            assert!(result["definition"].is_object());
            let alias = handle_request(Request::TypesResolveAlias { alias: "not-an-alias".into() }).await.unwrap();
            assert!(alias.is_null());
            let versions = handle_request(Request::TypesListVersions { type_id: "missing".into() }).await.unwrap();
            assert_eq!(versions, json!([]));
        });
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dotted_type_rpc_handlers_cover_aliases_versions_nulls_and_ordering() {
        let _guard = TEST_DB_MUTEX.blocking_lock();
        let path =
            std::env::temp_dir().join(format!("ark-phase2-rpc-matrix-{}.db", std::process::id()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            handle_request(Request::Init {
                db_path: path.to_string_lossy().into_owned(),
            })
            .await
            .unwrap();
            for id in ["rpc-z", "rpc-a"] {
                handle_request(Request::UpsertObjectType {
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
                })
                .await
                .unwrap();
            }
            with_conn(|conn| {
                ark_core::type_registry::register_alias(
                    conn,
                    &ark_core::type_registry::AliasRecord {
                        alias: "rpc.alias".into(),
                        canonical_type_id: "rpc-a".into(),
                        created_at: "a".into(),
                    },
                )
            })
            .unwrap();
            let list = handle_request(Request::TypesList).await.unwrap();
            let list_ids = list
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["typeId"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert!(list_ids.windows(2).all(|w| w[0] <= w[1]));
            let alias_get = handle_request(Request::TypesGet {
                type_id: "rpc.alias".into(),
                version: None,
            })
            .await
            .unwrap();
            assert_eq!(alias_get["summary"]["typeId"], "rpc-a");
            let alias_versions = handle_request(Request::TypesListVersions {
                type_id: "rpc.alias".into(),
            })
            .await
            .unwrap();
            assert!(!alias_versions.as_array().unwrap().is_empty());
            assert!(handle_request(Request::TypesGet {
                type_id: "unknown".into(),
                version: None
            })
            .await
            .unwrap()
            .is_null());
            assert_eq!(
                handle_request(Request::TypesResolveAlias {
                    alias: "unknown".into()
                })
                .await
                .unwrap(),
                Value::Null
            );
            assert_eq!(
                handle_request(Request::TypesListVersions {
                    type_id: "unknown".into()
                })
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

    #[tokio::test]
    async fn canonical_upsert_object_rpc_persists_registered_identity() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        handle_request(Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), canonical_task_props()),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        })
        .await
        .unwrap();
        let object = with_conn(|conn| db::get_object(conn, "canonical-task-ingress"))
            .unwrap()
            .unwrap();
        assert_eq!(object.type_id, "com.kosmos.task");
        assert_eq!(object.type_version, "1.0.0");
        assert_eq!(object.props_json["status"], "todo");
    }

    #[tokio::test]
    async fn canonical_upsert_object_rpc_rejects_omitted_version_without_side_effects() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        let result = handle_request(Request::UpsertObject {
            object: canonical_task_object(None, canonical_task_props()),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:canonical_version_required"
        );
        let object_count = with_conn(|conn| {
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
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let mut props = canonical_task_props();
        props["unexpected"] = json!(true);
        let result = handle_request(Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), props),
            expected_snapshot: None,
            device_id: Some("device-canonical".to_string()),
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:canonical_field:/unexpected"
        );
        let (objects, versions) = with_conn(|conn| {
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
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        let mut object = canonical_task_object(Some("1.0.0"), canonical_task_props());
        object.type_id = "task_obj".to_string();
        let result = handle_request(Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: None,
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:legacy_alias_new_write"
        );
    }

    #[tokio::test]
    async fn integration_rpc_rejects_revocation_without_local_authorization() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
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

        let error = handle_request(request).await.unwrap_err();
        assert_eq!(error, "node is not authorized");
    }

    #[tokio::test]
    async fn integration_rpc_rejects_stale_refresh_lease_fence() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        handle_request(Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().into_owned(),
        })
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
        handle_request(node).await.unwrap();

        let acquire = |expected_fencing_token| Request::IntegrationAcquireRefreshLease {
            integration_id: "refresh-integration".into(),
            holder_node_id: "refresh-node".into(),
            credential_generation: 1,
            now_ms: 1_000,
            ttl_ms: 100,
            expected_fencing_token,
            device_id: "refresh-node".into(),
        };
        let lease = handle_request(acquire(0)).await.unwrap();
        assert_eq!(lease["fencing_token"], 1);
        let error = handle_request(acquire(0)).await.unwrap_err();
        assert!(error.contains("expected_fencing_token"));
    }

    #[tokio::test]
    async fn integration_issuer_key_lookup_rejects_wrong_space() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        handle_request(Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
        setup_sync_with_capturing_transport(get_shared_conn().unwrap()).await;

        let error = handle_request(Request::IntegrationLookupIssuerEncryptionKey {
            space_id: "wrong-space".into(),
            integration_id: "integration-a".into(),
            recipient_node_id: "recipient".into(),
            issuer_node_id: "issuer".into(),
            credential_generation: 1,
            expected_issuer_key_id: "sha256:issuer".into(),
        })
        .await
        .unwrap_err();
        assert_eq!(error, "integration lookup requested for the wrong space");
        let prepublish_error = handle_request(Request::IntegrationLookupIssuerEncryptionKeyForPublish {
            space_id: "wrong-space".into(),
            integration_id: "integration-a".into(),
            recipient_node_id: "recipient".into(),
            issuer_node_id: "issuer".into(),
            expected_issuer_key_id: "sha256:issuer".into(),
        })
        .await
        .unwrap_err();
        assert_eq!(prepublish_error, "integration lookup requested for the wrong space");
        *SYNC.lock().await = None;
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
}
