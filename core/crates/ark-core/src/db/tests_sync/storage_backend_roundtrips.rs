// -----------------------------------------------------------------------
// SqliteStorageBackend roundtrip tests (AC3)
// -----------------------------------------------------------------------

struct TestBackend {
    backend: SqliteStorageBackend,
    _database: ark_store::test_support::TestDatabase,
}

impl std::ops::Deref for TestBackend {
    type Target = SqliteStorageBackend;

    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

impl TestBackend {
    fn database(&self) -> &ark_store::test_support::TestDatabase {
        &self._database
    }
}

async fn make_backend() -> TestBackend {
    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "device-under-test",
    )
    .await
    .expect("production writer opens for test");
    backend.set_device_id("device-under-test").unwrap();
    TestBackend {
        backend,
        _database: database,
    }
}

fn sync_tracked_app(id: &str) -> SyncEntity {
    let tracked_app = make_tracked_app(id);
    let value = serde_json::to_value(&tracked_app).unwrap();
    let Value::Object(mut map) = value else {
        panic!("expected JSON object, got {value:?}")
    };
    map.remove("id");
    SyncEntity {
        entity_type: "tracked_app".to_string(),
        id: id.to_string(),
        data: map,
        hlc: "2026-01-01T00:00:00.000Z:000001:peer-a".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

fn sync_usage_session(id: &str, tracked_app_id: &str) -> SyncEntity {
    let session = make_usage_session(id, tracked_app_id);
    let value = serde_json::to_value(&session).unwrap();
    let Value::Object(mut map) = value else {
        panic!("expected JSON object, got {value:?}")
    };
    map.remove("id");
    SyncEntity {
        entity_type: "usage_session".to_string(),
        id: id.to_string(),
        data: map,
        hlc: "2026-01-01T00:00:00.000Z:000002:peer-a".to_string(),
        deleted: None,
        origin_device_id: Some("peer-a".to_string()),
        origin_seq: Some(2),
    }
}

fn sync_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> SyncEntity {
    let event = make_usage_event(id, tracked_app_id, session_id);
    let value = serde_json::to_value(&event).unwrap();
    let Value::Object(mut map) = value else {
        panic!("expected JSON object, got {value:?}")
    };
    map.remove("id");
    SyncEntity {
        entity_type: "usage_event".to_string(),
        id: id.to_string(),
        data: map,
        hlc: "2026-01-01T00:00:00.000Z:000003:peer-a".to_string(),
        deleted: None,
        origin_device_id: Some("peer-a".to_string()),
        origin_seq: Some(3),
    }
}

fn sync_object(id: &str, type_id: &str, title: &str) -> SyncEntity {
    let object = make_object(id, type_id, title);
    let value = serde_json::to_value(&object).unwrap();
    let Value::Object(mut map) = value else {
        panic!("expected JSON object, got {value:?}")
    };
    map.remove("id");
    SyncEntity {
        entity_type: "object".to_string(),
        id: id.to_string(),
        data: map,
        hlc: "2026-01-01T00:00:00.000Z:000010:peer-a".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

#[tokio::test]
async fn storage_backend_roundtrip_core_note() {
    let backend = make_backend().await;
    let entity = sync_object("tbk1", "core.note", "Roundtrip");
    backend.apply_entity(&entity).await.unwrap();

    let empty_vector: VersionVector = std::collections::HashMap::new();
    let loaded = backend.load_entities(&empty_vector).await.unwrap();
    let found = loaded
        .iter()
        .find(|e| e.id == "tbk1")
        .expect("inserted core.note should be loaded");
    assert_eq!(found.entity_type, "object");
    assert_eq!(
        found.data.get("title").and_then(|v| v.as_str()),
        Some("Roundtrip")
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_backend_rejects_invalid_sync_payload() {
    let backend = make_backend().await;
    let mut entity = sync_object("tbk-invalid", "core.note", "Invalid");
    entity.data.insert("title".to_string(), json!(123));

    let err = backend
        .apply_entity(&entity)
        .await
        .expect_err("invalid payload should return an apply error");
    assert!(
        err.contains("invalid type") || err.contains("expected"),
        "unexpected error: {err}",
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_backend_rejects_unknown_sync_entity_type() {
    let backend = make_backend().await;
    let mut entity = sync_object("tbk-unknown", "core.note", "Unknown");
    entity.entity_type = "unknown_entity".to_string();

    let err = backend
        .apply_entity(&entity)
        .await
        .expect_err("unknown entity type should return an apply error");
    assert!(
        err.contains("unknown sync entity type"),
        "unexpected error: {err}"
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_backend_roundtrip_usage_entities() {
    let backend = make_backend().await;
    backend
        .apply_entity(&sync_tracked_app("app-sync"))
        .await
        .unwrap();
    backend
        .apply_entity(&sync_usage_session("session-sync", "app-sync"))
        .await
        .unwrap();
    backend
        .apply_entity(&sync_usage_event(
            "event-sync",
            "app-sync",
            Some("session-sync"),
        ))
        .await
        .unwrap();

    let empty_vector: VersionVector = std::collections::HashMap::new();
    let loaded = backend.load_entities(&empty_vector).await.unwrap();

    let tracked_app = loaded
        .iter()
        .find(|e| e.entity_type == "tracked_app" && e.id == "app-sync")
        .expect("inserted tracked app should be loaded");
    assert_eq!(
        tracked_app.data.get("displayName").and_then(|v| v.as_str()),
        Some("Demo App")
    );

    let session = loaded
        .iter()
        .find(|e| e.entity_type == "usage_session" && e.id == "session-sync")
        .expect("inserted usage session should be loaded");
    assert_eq!(
        session.data.get("trackedAppId").and_then(|v| v.as_str()),
        Some("app-sync")
    );

    let event = loaded
        .iter()
        .find(|e| e.entity_type == "usage_event" && e.id == "event-sync")
        .expect("inserted usage event should be loaded");
    assert_eq!(
        event.data.get("kind").and_then(|v| v.as_str()),
        Some("foreground")
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_backend_pages_large_usage_history() {
    let backend = make_backend().await;
    backend
        .apply_entity(&sync_tracked_app("app-paged"))
        .await
        .unwrap();
    for index in 0..250 {
        let mut event = sync_usage_event(&format!("event-paged-{index:03}"), "app-paged", None);
        event.origin_seq = Some(index + 1);
        backend.apply_entity(&event).await.unwrap();
    }
    let vector = VersionVector::new();
    let mut offset = 0;
    let mut usage_event_ids = std::collections::HashSet::new();

    loop {
        let page = backend
            .load_entities_page(&vector, offset, 100)
            .await
            .unwrap();
        assert!(page.len() <= 100);
        if page.is_empty() {
            break;
        }
        offset += page.len();
        usage_event_ids.extend(
            page.into_iter()
                .filter(|entity| entity.entity_type == "usage_event")
                .map(|entity| entity.id),
        );
    }

    assert_eq!(usage_event_ids.len(), 250);
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_backend_roundtrip_object_entities() {
    let backend = make_backend().await;
    backend
        .apply_entity(&sync_object("obj-sync-a", "core.note", "Ark note"))
        .await
        .unwrap();
    backend
        .apply_entity(&sync_object("obj-sync-b", "core.game", "Ark game"))
        .await
        .unwrap();
    let empty_vector: VersionVector = std::collections::HashMap::new();
    let loaded = backend.load_entities(&empty_vector).await.unwrap();

    let note = loaded
        .iter()
        .find(|e| e.entity_type == "object" && e.id == "obj-sync-a")
        .expect("inserted object should be loaded");
    assert_eq!(
        note.data.get("title").and_then(|v| v.as_str()),
        Some("Ark note")
    );

    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
