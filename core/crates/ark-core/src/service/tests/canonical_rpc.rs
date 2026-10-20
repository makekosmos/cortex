use super::*;
use std::path::Path;

#[path = "canonical_rpc/object_hlc.rs"]
mod object_hlc;
#[path = "canonical_rpc/task_idempotency.rs"]
mod task_idempotency;

fn remove_test_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let file = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            path.with_file_name(format!(
                "{}{}",
                path.file_name().unwrap().to_string_lossy(),
                suffix
            ))
        };
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "failed to remove test database file {}: {error}",
                file.display()
            ),
        }
    }
}

#[test]
fn dotted_type_rpc_hits_real_handler_and_omitted_upsert_resolves_current() {
    let state = test_state();
    let path = std::env::temp_dir().join(format!("ark-phase2-rpc-{}.db", std::process::id()));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let init = Request::Init {
            db_path: path.to_string_lossy().into_owned(),
        };
        handle_request(&state, init).await.unwrap();
        let type_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object_type",
            "object_type": {
                "id": "rpc-phase2", "name": "RPC", "schemaJson": "{}",
                "uiSchemaJson": "{}", "createdAt": "c", "updatedAt": "u",
                "systemLocked": false
            }
        }))
        .unwrap();
        handle_request(&state, type_request).await.unwrap();
        let object_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object",
            "object": {
                "id": "rpc-object", "typeId": "rpc-phase2", "title": "x",
                "contentJson": {}, "propsJson": {}, "createdAt": "c", "updatedAt": "u",
                "deletedAt": null
            }
        }))
        .unwrap();
        handle_request(&state, object_request).await.unwrap();
        let stored = handle_request(
            &state,
            Request::GetObject {
                id: "rpc-object".into(),
            },
        )
        .await
        .unwrap();
        assert!(stored["typeVersion"]
            .as_str()
            .is_some_and(|version| version.starts_with("0.0.0+legacy.")));
        let mut event_rx = crate::events::subscribe();
        let before_objects = handle_request(&state, Request::ListObjects).await.unwrap();
        let before_versions = handle_request(
            &state,
            Request::GetSyncKv {
                key: "lan_sync.version_vector".into(),
            },
        )
        .await
        .unwrap();
        let unknown_request: Request = serde_json::from_value(json!({
            "operation": "upsert_object",
            "object": {
                "id": "rpc-unknown", "typeId": "rpc-phase2",
                "typeVersion": "9.9.9", "title": "unknown", "contentJson": {},
                "propsJson": {}, "createdAt": "c", "updatedAt": "u", "deletedAt": null
            }
        }))
        .unwrap();
        assert!(handle_request(&state, unknown_request).await.is_err());
        assert_eq!(
            handle_request(&state, Request::ListObjects).await.unwrap(),
            before_objects,
            "rejected object is not visible through the public list API"
        );
        assert_eq!(
            handle_request(
                &state,
                Request::GetSyncKv {
                    key: "lan_sync.version_vector".into(),
                },
            )
            .await
            .unwrap(),
            before_versions,
            "rejected object does not advance the sync version vector"
        );
        let failed_object_event = received_upsert_for(&mut event_rx, "rpc-unknown");
        assert!(
            !failed_object_event,
            "failed object upsert must not emit object_upserted for rpc-unknown"
        );
        let result = handle_request(
            &state,
            Request::TypesGet {
                type_id: "rpc-phase2".into(),
                version: None,
            },
        )
        .await
        .unwrap();
        assert!(result["summary"].is_object());
        assert!(result["definition"].is_object());
        let alias = handle_request(
            &state,
            Request::TypesResolveAlias {
                alias: "not-an-alias".into(),
            },
        )
        .await
        .unwrap();
        assert!(alias.is_null());
        let versions = handle_request(
            &state,
            Request::TypesListVersions {
                type_id: "missing".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(versions, json!([]));
        shutdown_test_state(&state).await;
    });
    drop(runtime);
    drop(state);
    remove_test_database(&path);
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
        shutdown_test_state(&state).await;
    });
    drop(runtime);
    drop(state);
    remove_test_database(&path);
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
async fn canonical_upsert_object_rpc_rejects_omitted_version_without_side_effects() {
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
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(None, canonical_task_props()),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:canonical_version_required"
    );
    assert_eq!(
        handle_request(&state, Request::ListObjects).await.unwrap(),
        json!([]),
        "rejected object is not visible through the public list API"
    );
    fixture.shutdown().await;
}

#[tokio::test]
async fn canonical_upsert_object_rpc_rejects_invalid_payload_without_sync_mutation() {
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

    let mut props = canonical_task_props();
    props["unexpected"] = json!(true);
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(Some("0.1.0"), props),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:canonical_field:/unexpected"
    );
    assert_eq!(
        handle_request(&state, Request::ListObjects).await.unwrap(),
        json!([]),
        "invalid canonical payload leaves object storage unchanged"
    );
    assert_eq!(
        handle_request(
            &state,
            Request::GetSyncKv {
                key: "lan_sync.version_vector".into(),
            },
        )
        .await
        .unwrap(),
        Value::Null,
        "invalid canonical payload does not create sync state"
    );
    fixture.shutdown().await;
}

#[tokio::test]
async fn canonical_upsert_object_rpc_rejects_legacy_alias_as_new_write() {
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
    let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
    object.type_id = "note_obj".to_string();
    object.type_version = Some("1.0.0".to_string());
    let result = handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            operation_id: None,
            device_id: None,
        },
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "canonical_ingress:invalid_request:legacy_alias_new_write"
    );
    fixture.shutdown().await;
}
