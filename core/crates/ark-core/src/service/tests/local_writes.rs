use super::*;

#[tokio::test]
async fn local_object_and_usage_writes_record_sync_state() {
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

    let object = ArkObjectWrite {
        id: "obj-local-write".to_string(),
        type_id: "com.kosmos.game".to_string(),
        type_version: Some("1.0.0".to_string()),
        title: "Local Game".to_string(),
        content_json: json!({}),
        props_json: json!({
            "playStatus": null,
            "userRating": null,
            "genres": [],
            "platforms": [],
            "released": null,
            "description": null,
            "extensions": {}
        }),
        created_at: "2026-04-24T00:00:00.000Z".to_string(),
        updated_at: "2026-04-24T00:00:00.000Z".to_string(),
        deleted_at: None,
    };
    handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let tracked_app = TrackedApp {
        id: "app-local-write".to_string(),
        platform: "windows".to_string(),
        exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
        normalized_exe_path: "c:\\games\\demo\\demo.exe".to_string(),
        process_name: "demo.exe".to_string(),
        display_name: Some("Demo".to_string()),
        publisher: None,
        icon_ref: None,
        first_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
        last_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
    };
    handle_request(
        &state,
        Request::UpsertTrackedApp {
            tracked_app,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let session = UsageSession {
        id: "session-local-write".to_string(),
        tracked_app_id: "app-local-write".to_string(),
        device_id: "device-local".to_string(),
        device_name: "Device".to_string(),
        platform: "windows".to_string(),
        started_at: "2026-04-24T00:00:00.000Z".to_string(),
        ended_at: None,
        runtime_ms: 1000,
        foreground_ms: 1000,
        idle_ms: 0,
        window_title: Some("Demo".to_string()),
        process_name: "demo.exe".to_string(),
        exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
        pid_start: Some(1),
        pid_end: None,
        meta_json: json!({}),
    };
    handle_request(
        &state,
        Request::UpsertUsageSession {
            usage_session: session,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let event = UsageEvent {
        id: "event-local-write".to_string(),
        tracked_app_id: "app-local-write".to_string(),
        usage_session_id: Some("session-local-write".to_string()),
        device_id: "device-local".to_string(),
        device_name: "Device".to_string(),
        platform: "windows".to_string(),
        occurred_at: "2026-04-24T00:00:01.000Z".to_string(),
        kind: "foreground".to_string(),
        window_title: Some("Demo".to_string()),
        process_name: "demo.exe".to_string(),
        exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
        pid: Some(1),
        is_foreground: true,
        is_idle: false,
        meta_json: json!({}),
    };
    handle_request(
        &state,
        Request::UpsertUsageEvent {
            usage_event: event,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    handle_request(
        &state,
        Request::DeleteObject {
            id: "obj-local-write".to_string(),
            expected_snapshot: None,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let shared = get_shared_conn(&state).unwrap();
    let guard = shared.lock().unwrap();
    let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
        .unwrap()
        .expect("version vector should be stored");
    let vector: VersionVector = serde_json::from_str(&raw).unwrap();
    for id in ["obj-local-write", "app-local-write"] {
        assert!(
            vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-local")),
            "{id} should have a local HLC in the version vector",
        );
    }
    assert_eq!(
        vector.get("@usage:device-local").map(String::as_str),
        Some("2")
    );
    assert!(!vector.contains_key("session-local-write"));
    assert!(!vector.contains_key("event-local-write"));

    let tombstone_count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
            rusqlite::params!["obj-local-write", "object"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tombstone_count, 1);
}

#[tokio::test]
async fn local_object_type_and_link_writes_record_sync_state() {
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

    let timestamp = "2026-04-24T00:00:00.000Z".to_string();
    let object_type = ObjectType {
        id: "rpc-game-type".to_string(),
        name: "Game".to_string(),
        schema_json: "{}".to_string(),
        ui_schema_json: "{}".to_string(),
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        system_locked: false,
    };
    handle_request(
        &state,
        Request::UpsertObjectType {
            object_type: object_type.clone(),
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::UpsertObjectType {
            object_type: ObjectType {
                id: "empty_type_for_delete".to_string(),
                name: "Empty".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
                system_locked: false,
            },
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let source = ArkObjectWrite {
        id: "source-object".to_string(),
        type_id: object_type.id.clone(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "Source".to_string(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        deleted_at: None,
    };
    let target = ArkObjectWrite {
        id: "target-object".to_string(),
        type_id: object_type.id.clone(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "Target".to_string(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        deleted_at: None,
    };
    handle_request(
        &state,
        Request::UpsertObject {
            object: source,
            expected_snapshot: None,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::UpsertObject {
            object: target,
            expected_snapshot: None,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let link = ObjectLink {
        id: "link-local-write".to_string(),
        source_object_id: "source-object".to_string(),
        target_object_id: "target-object".to_string(),
        link_type: "related".to_string(),
        created_at: timestamp,
    };
    handle_request(
        &state,
        Request::UpsertObjectLink {
            object_link: link,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    handle_request(
        &state,
        Request::DeleteObjectLink {
            id: "link-local-write".to_string(),
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::DeleteObjectType {
            id: "empty_type_for_delete".to_string(),
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let shared = get_shared_conn(&state).unwrap();
    let guard = shared.lock().unwrap();
    let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
        .unwrap()
        .expect("version vector should be stored");
    let vector: VersionVector = serde_json::from_str(&raw).unwrap();
    for id in ["rpc-game-type", "link-local-write", "empty_type_for_delete"] {
        assert!(
            vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-local")),
            "{id} should have a local HLC in the version vector",
        );
    }

    let link_tombstone_count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
            rusqlite::params!["link-local-write", "object_link"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(link_tombstone_count, 1);

    let type_tombstone_count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
            rusqlite::params!["empty_type_for_delete", "object_type"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(type_tombstone_count, 1);
}
