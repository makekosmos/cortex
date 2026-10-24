use super::*;

#[tokio::test]
async fn local_object_and_usage_writes_record_sync_state() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    initialize_test_database(&state, db_path.to_string_lossy().to_string(),)
    .await
    .unwrap();

    let object = ArkObjectWrite {
        id: "obj-local-write".to_string(),
        type_id: "core.game".to_string(),
        type_version: Some("0.1.0".to_string()),
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
            operation_id: None,
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
            operation_id: None,
            device_id: Some("device-local".to_string()),
        },
    )
    .await
    .unwrap();

    let sync_state = ark_store::test_support::SyncPersistenceFixture::inspect(&db_path).unwrap();
    for id in ["obj-local-write", "app-local-write"] {
        assert!(
            sync_state
                .version_vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-local")),
            "{id} should have a local HLC in the version vector",
        );
    }
    assert_eq!(
        sync_state
            .version_vector
            .get("@usage:device-local")
            .map(String::as_str),
        Some("2")
    );
    assert!(!sync_state
        .version_vector
        .contains_key("session-local-write"));
    assert!(!sync_state.version_vector.contains_key("event-local-write"));
    assert!(sync_state
        .tombstones
        .iter()
        .any(|(id, entity_type)| id == "obj-local-write" && entity_type == "object"));
}

#[tokio::test]
async fn local_object_type_and_link_writes_record_sync_state() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    initialize_test_database(&state, db_path.to_string_lossy().to_string(),)
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
            operation_id: None,
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
            operation_id: None,
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

    let sync_state = ark_store::test_support::SyncPersistenceFixture::inspect(&db_path).unwrap();
    for id in ["rpc-game-type", "link-local-write", "empty_type_for_delete"] {
        assert!(
            sync_state
                .version_vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-local")),
            "{id} should have a local HLC in the version vector",
        );
    }

    assert!(sync_state
        .tombstones
        .iter()
        .any(|(id, entity_type)| id == "link-local-write" && entity_type == "object_link"));
    assert!(sync_state.tombstones.iter().any(|(id, entity_type)| {
        id == "empty_type_for_delete" && entity_type == "object_type"
    }));
}
