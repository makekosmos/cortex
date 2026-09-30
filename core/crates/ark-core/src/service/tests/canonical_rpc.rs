use super::*;

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
