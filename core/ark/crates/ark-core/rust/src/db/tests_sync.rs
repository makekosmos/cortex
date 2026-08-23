    // -----------------------------------------------------------------------
    // SqliteStorageBackend roundtrip tests (AC3)
    // -----------------------------------------------------------------------

    fn make_backend() -> SqliteStorageBackend {
        let conn = setup_db();
        let shared = Arc::new(Mutex::new(conn));
        let backend = SqliteStorageBackend::new(shared);
        backend.set_device_id("device-under-test").unwrap();
        backend
    }

    fn sync_todo(id: &str, title: &str) -> SyncEntity {
        let todo = make_todo(id, title);
        let value = serde_json::to_value(&todo).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "todo".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    fn sync_tracked_app(id: &str) -> SyncEntity {
        let tracked_app = make_tracked_app(id);
        let value = serde_json::to_value(&tracked_app).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
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
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
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
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
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
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
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

    fn sync_object_link(id: &str, source_object_id: &str, target_object_id: &str) -> SyncEntity {
        let object_link = make_object_link(id, source_object_id, target_object_id);
        let value = serde_json::to_value(&object_link).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "object_link".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000011:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_todo() {
        let backend = make_backend();
        let entity = sync_todo("tbk1", "Roundtrip");
        backend.apply_entity(&entity).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk1")
            .expect("inserted todo should be loaded");
        assert_eq!(found.entity_type, "object");
        assert_eq!(
            found.data.get("title").and_then(|v| v.as_str()),
            Some("Roundtrip")
        );
    }

    #[tokio::test]
    async fn storage_backend_rejects_invalid_sync_payload() {
        let backend = make_backend();
        let mut entity = sync_todo("tbk-invalid", "Invalid");
        entity.data.insert("title".to_string(), json!(123));

        let err = backend
            .apply_entity(&entity)
            .await
            .expect_err("invalid payload should return an apply error");
        assert!(
            err.contains("invalid type") || err.contains("expected"),
            "unexpected error: {err}",
        );
    }

    #[tokio::test]
    async fn storage_backend_rejects_unknown_sync_entity_type() {
        let backend = make_backend();
        let mut entity = sync_todo("tbk-unknown", "Unknown");
        entity.entity_type = "unknown_entity".to_string();

        let err = backend
            .apply_entity(&entity)
            .await
            .expect_err("unknown entity type should return an apply error");
        assert!(
            err.contains("unknown sync entity type"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_usage_entities() {
        let backend = make_backend();
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
        let loaded = backend.load_entities(&empty_vector).await;

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
    }

    #[tokio::test]
    async fn storage_backend_pages_large_usage_history() {
        let conn = setup_db();
        upsert_tracked_app(&conn, &make_tracked_app("app-paged")).unwrap();
        for index in 0..250 {
            upsert_usage_event(
                &conn,
                &make_usage_event(&format!("event-paged-{index:03}"), "app-paged", None),
            )
            .unwrap();
        }
        let backend = SqliteStorageBackend::new(Arc::new(Mutex::new(conn)));
        backend.set_device_id("device-paged").unwrap();
        let vector = VersionVector::new();
        let mut offset = 0;
        let mut usage_event_ids = std::collections::HashSet::new();

        loop {
            let page = backend.load_entities_page(&vector, offset, 100).await;
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
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_object_entities() {
        let backend = make_backend();
        backend
            .apply_entity(&sync_object("obj-sync-a", "note_obj", "Ark note"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_object("obj-sync-b", "game_obj", "Ark game"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_object_link("link-sync", "obj-sync-a", "obj-sync-b"))
            .await
            .unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;

        let note = loaded
            .iter()
            .find(|e| e.entity_type == "object" && e.id == "obj-sync-a")
            .expect("inserted object should be loaded");
        assert_eq!(
            note.data.get("title").and_then(|v| v.as_str()),
            Some("Ark note")
        );

        let link = loaded
            .iter()
            .find(|e| e.entity_type == "object_link" && e.id == "link-sync")
            .expect("inserted object link should be loaded");
        assert_eq!(
            link.data.get("sourceObjectId").and_then(|v| v.as_str()),
            Some("obj-sync-a")
        );
    }

    #[tokio::test]
    async fn storage_backend_delete_removes_entity() {
        let backend = make_backend();
        let entity = sync_todo("tbk2", "To delete");
        backend.apply_entity(&entity).await.unwrap();

        // Now apply a tombstone.
        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk2".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk2")
            .expect("deleted entity should be emitted as a tombstone");
        assert_eq!(found.entity_type, "object");
        assert_eq!(found.deleted, Some(true));
        assert_eq!(found.hlc, "2026-01-02T00:00:00.000Z:000001:peer-a");
        assert!(found.data.is_empty());
    }

    #[tokio::test]
    async fn storage_backend_tombstone_survives_backend_recreation() {
        let conn = setup_db();
        let shared = Arc::new(Mutex::new(conn));
        let backend = SqliteStorageBackend::new(shared.clone());

        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk-recreated".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000002:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let recreated = SqliteStorageBackend::new(shared);
        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = recreated.load_entities(&empty_vector).await;
        assert!(
            loaded
                .iter()
                .any(|e| e.id == "tbk-recreated" && e.deleted == Some(true)),
            "tombstone should survive recreating the storage backend",
        );
    }

    #[tokio::test]
    async fn storage_backend_live_entity_clears_tombstone() {
        let backend = make_backend();
        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk-resurrected".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let mut live = sync_todo("tbk-resurrected", "Live again");
        live.hlc = "2026-01-03T00:00:00.000Z:000001:peer-a".to_string();
        backend.apply_entity(&live).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let matching: Vec<&SyncEntity> = loaded
            .iter()
            .filter(|e| e.id == "tbk-resurrected")
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].deleted, None);
        assert_eq!(
            matching[0].data.get("title").and_then(|v| v.as_str()),
            Some("Live again")
        );
    }

    #[test]
    fn local_sync_version_bump_persists_hlc_for_entity() {
        let conn = setup_db();
        let first =
            bump_sync_version_vector(&conn, "object", "obj-local", "device-local", false).unwrap();
        let second =
            bump_sync_version_vector(&conn, "object", "obj-local", "device-local", false).unwrap();

        assert!(HLC::is_newer(&second, &first) || second > first);
        let raw = get_sync_kv(&conn, VERSION_VECTOR_KEY)
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        assert_eq!(vector.get("obj-local"), Some(&second));
        assert!(second.ends_with(":device-local"));
    }

    #[test]
    fn local_sync_tombstone_is_persisted_and_can_be_cleared() {
        let conn = setup_db();
        record_sync_tombstone(
            &conn,
            "object",
            "obj-deleted",
            "2026-01-02T00:00:00.000Z:000001:device-local",
        )
        .unwrap();

        let tombstones = load_sync_tombstones(&conn).unwrap();
        assert!(
            tombstones.iter().any(|entity| {
                entity.entity_type == "object"
                    && entity.id == "obj-deleted"
                    && entity.deleted == Some(true)
            }),
            "local delete should leave a durable tombstone",
        );

        delete_sync_tombstone(&conn, "obj-deleted").unwrap();
        let tombstones = load_sync_tombstones(&conn).unwrap();
        assert!(
            tombstones.iter().all(|entity| entity.id != "obj-deleted"),
            "local live upsert should be able to clear prior tombstone",
        );
    }

    #[tokio::test]
    async fn storage_backend_delete_usage_entity_removes_entity() {
        let backend = make_backend();
        backend
            .apply_entity(&sync_tracked_app("app-del"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session("session-del", "app-del"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_event(
                "event-del",
                "app-del",
                Some("session-del"),
            ))
            .await
            .unwrap();

        let tombstone = SyncEntity {
            entity_type: "usage_event".to_string(),
            id: "event-del".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: Some("peer-a".to_string()),
            origin_seq: Some(4),
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "event-del")
            .expect("deleted usage event should be emitted as a tombstone");
        assert_eq!(found.entity_type, "usage_event");
        assert_eq!(found.deleted, Some(true));
        assert!(found.data.is_empty());
    }

    #[tokio::test]
    async fn storage_backend_kv_roundtrip() {
        let backend = make_backend();
        assert_eq!(backend.get_kv("foo").await, None);
        backend.set_kv("foo", "bar").await;
        assert_eq!(backend.get_kv("foo").await, Some("bar".to_string()));
    }

    #[tokio::test]
    async fn storage_backend_uses_stored_hlc_when_present() {
        let backend = make_backend();
        let entity = sync_todo("tbk3", "With HLC");
        backend.apply_entity(&entity).await.unwrap();

        let mut vector: VersionVector = std::collections::HashMap::new();
        vector.insert(
            "tbk3".to_string(),
            "2026-03-01T00:00:00.000Z:000005:peer-b".to_string(),
        );

        let loaded = backend.load_entities(&vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk3")
            .expect("entity present");
        assert_eq!(
            found.hlc,
            "2026-03-01T00:00:00.000Z:000005:peer-b".to_string(),
            "load_entities should prefer HLC from the passed version vector",
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2: hold-and-replay (sync_pending_objects)
    // -----------------------------------------------------------------------

    fn phase2_make_object_type(id: &str, name: &str) -> ObjectType {
        ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        }
    }

    fn phase2_make_sync_entity_object(id: &str, type_id: &str, title: &str) -> SyncEntity {
        let mut data = serde_json::Map::new();
        data.insert("typeId".to_string(), Value::String(type_id.to_string()));
        data.insert(
            "typeVersion".to_string(),
            Value::String("0.0.0-legacy".to_string()),
        );
        data.insert("title".to_string(), Value::String(title.to_string()));
        data.insert("contentJson".to_string(), json!({}));
        data.insert("propsJson".to_string(), json!({}));
        data.insert(
            "createdAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        data.insert(
            "updatedAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data,
            hlc: "2026-01-01T00:00:00.000Z:000001:test".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[test]
    fn phase2_is_object_type_known_false_when_missing() {
        let conn = setup_db();
        assert!(!is_object_type_known(&conn, "nonexistent").unwrap());
    }

    #[test]
    fn phase2_is_object_type_known_true_after_upsert() {
        let conn = setup_db();
        let object_type = phase2_make_object_type("type_x", "Type X");
        upsert_object_type(&conn, &object_type).unwrap();
        assert!(is_object_type_known(&conn, "type_x").unwrap());
    }

    #[test]
    fn phase2_insert_pending_object_persists_payload() {
        let conn = setup_db();
        let entity = phase2_make_sync_entity_object("obj-1", "future_type", "Pending object");
        insert_pending_object(&conn, &entity, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
        assert_eq!(count_pending_for_type(&conn, "other_type").unwrap(), 0);
    }

    #[test]
    fn phase2_insert_pending_overwrites_same_id() {
        // Если sync приносит обновлённую version того же object'а, REPLACE'ит,
        // не дублирует.
        let conn = setup_db();
        let e1 = phase2_make_sync_entity_object("obj-1", "future_type", "First");
        let e2 = phase2_make_sync_entity_object("obj-1", "future_type", "Second");
        insert_pending_object(&conn, &e1, "future_type").unwrap();
        insert_pending_object(&conn, &e2, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
    }

    #[test]
    fn phase2_replay_runs_when_type_appears() {
        let conn = setup_db();
        // 1. Pending object для типа, который ещё не существует.
        let entity = phase2_make_sync_entity_object("obj-1", "type_late", "Awaiting type");
        insert_pending_object(&conn, &entity, "type_late").unwrap();
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 1);

        // Object table должна быть пустой.
        let objects_before = list_objects(&conn).unwrap();
        assert_eq!(objects_before.len(), 0);

        // 2. Создаём тип — должен сработать auto-replay.
        let object_type = phase2_make_object_type("type_late", "Late Type");
        upsert_object_type(&conn, &object_type).unwrap();

        // 3. Pending очищен, object материализован.
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 0);
        let objects_after = list_objects(&conn).unwrap();
        assert_eq!(objects_after.len(), 1);
        assert_eq!(objects_after[0].id, "obj-1");
        assert_eq!(objects_after[0].title, "Awaiting type");
        assert_eq!(objects_after[0].type_id, "type_late");
    }

    #[test]
    fn phase2_replay_handles_multiple_pending_same_type() {
        let conn = setup_db();
        for i in 0..5 {
            let e = phase2_make_sync_entity_object(
                &format!("obj-{i}"),
                "batch_type",
                &format!("Obj {i}"),
            );
            insert_pending_object(&conn, &e, "batch_type").unwrap();
        }
        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 5);

        upsert_object_type(&conn, &phase2_make_object_type("batch_type", "Batch")).unwrap();

        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 0);
        assert_eq!(list_objects(&conn).unwrap().len(), 5);
    }

    #[test]
    fn phase2_replay_only_targets_matching_type() {
        let conn = setup_db();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-a", "type_a", "A"),
            "type_a",
        )
        .unwrap();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-b", "type_b", "B"),
            "type_b",
        )
        .unwrap();

        upsert_object_type(&conn, &phase2_make_object_type("type_a", "Type A")).unwrap();

        // Только obj-a replayed; obj-b всё ещё в pending.
        assert_eq!(count_pending_for_type(&conn, "type_a").unwrap(), 0);
        assert_eq!(count_pending_for_type(&conn, "type_b").unwrap(), 1);
        let objects = list_objects(&conn).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "obj-a");
    }

    // --- Hardening (2026-05-18) ---

    #[test]
    fn check_integrity_passes_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        check_integrity(&conn).expect("пустая DB должна проходить integrity_check");
    }

    #[test]
    fn check_integrity_passes_after_init_schema() {
        let conn = setup_db();
        check_integrity(&conn).expect("DB после init_schema должна быть целостной");
    }

    #[test]
    fn init_schema_fails_on_corrupted_db() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // Создаём валидную DB и наполняем данными (нужно ≥ 1 data page чтобы
        // повредить не header).
        {
            let conn = open_db(path.to_str().unwrap()).unwrap();
            init_schema(&conn).unwrap();
            let object_type = make_object_type("corruption_fixture_type", "Note");
            upsert_object_type(&conn, &object_type).unwrap();
            for i in 0..100 {
                let mut obj = make_object(
                    &format!("obj-{i}"),
                    "corruption_fixture_type",
                    &format!("Title {i}"),
                );
                obj.props_json = json!({ "n": i, "padding": "x".repeat(200) });
                upsert_object(&conn, &obj).unwrap();
            }
        }
        // Портим data pages (offset 8192+, после header page и schema page) —
        // SQLite header останется валидным, integrity_check обнаружит
        // повреждённые btree pages.
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(8192)).unwrap();
            file.write_all(&[0xFF; 4096]).unwrap();
            file.flush().unwrap();
        }
        // Открываем снова. open_db может пройти (header интактен) или fail
        // на PRAGMA journal_mode. Если открыт — init_schema fail'ит на
        // integrity_check. Любой путь — fail-loud.
        let open_result = open_db(path.to_str().unwrap());
        if let Ok(conn) = open_result {
            let result = init_schema(&conn);
            assert!(
                result.is_err(),
                "init_schema должен fail при corrupted DB; результат: {result:?}"
            );
            let err = result.unwrap_err();
            assert!(
                err.contains("integrity")
                    || err.contains("corruption")
                    || err.contains("malformed"),
                "ошибка должна упоминать integrity/corruption/malformed, получили: {err}"
            );
        }
        // else: open_db уже fail'ил — тоже acceptable fail-loud path.
    }

    #[test]
    fn backup_to_file_creates_valid_copy() {
        let conn = setup_db();
        // Положим test данные.
        let object_type = make_object_type("backup_fixture_type", "Note");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-backup", "backup_fixture_type", "Backup test");
        object.props_json = json!({"tag": "test"});
        upsert_object(&conn, &object).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file(&conn, dest_str).expect("backup должен пройти");

        assert!(dest.exists(), "файл backup'а должен существовать");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup должен содержать оригинальный объект"
        );
        assert_eq!(objects[0].id, "obj-backup");
        assert_eq!(objects[0].title, "Backup test");
    }

    #[test]
    fn backup_to_file_chunked_creates_valid_copy_via_separate_connection() {
        let tmp = tempfile::tempdir().unwrap();
        let src_path = tmp.path().join("ark.db");
        let src_str = src_path.to_str().unwrap();
        {
            // Файловый source: chunked backup открывает его по пути отдельным
            // коннекшном (а не из переданного &Connection).
            let conn = open_db(src_str).unwrap();
            init_schema(&conn).unwrap();
            upsert_object_type(
                &conn,
                &make_object_type("backup_chunk_fixture_type", "Note"),
            )
            .unwrap();
            upsert_object(
                &conn,
                &make_object("obj-chunked", "backup_chunk_fixture_type", "Chunked backup"),
            )
            .unwrap();
        }

        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file_chunked(src_str, dest_str, 4, std::time::Duration::from_millis(0))
            .expect("chunked backup должен пройти");

        assert!(dest.exists(), "файл backup'а должен существовать");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup должен содержать оригинальный объект"
        );
        assert_eq!(objects[0].id, "obj-chunked");
        assert_eq!(objects[0].title, "Chunked backup");
    }

    // Фаза B — тесты вложенности (RED до замены BEGIN→SAVEPOINT).

    #[test]
    fn batch_upsert_todos_nests_in_outer_transaction() {
        let conn = setup_db();
        // Открываем внешнюю транзакцию — имитируем вызов из with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let todos = vec![
            make_todo("bt1", "Nested todo A"),
            make_todo("bt2", "Nested todo B"),
        ];
        // До Фазы B падает: "cannot start a transaction within a transaction".
        // После Фазы B (SAVEPOINT) должно пройти без ошибки.
        batch_upsert_todos(&conn, &todos)
            .expect("batch_upsert_todos должен работать внутри внешней транзакции");
        conn.execute_batch("COMMIT").unwrap();
        // Проверяем, что todo действительно записаны.
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 2, "оба todo должны быть записаны");
    }

    #[test]
    fn upsert_object_type_nests_in_outer_transaction() {
        let conn = setup_db();
        let ot = make_object_type("ot-nested", "Тип вложенный");
        // Открываем внешнюю транзакцию — имитируем вызов из with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        // До Фазы B падает через replay_pending_for_type → BEGIN IMMEDIATE:
        // "cannot start a transaction within a transaction".
        // После Фазы B (SAVEPOINT) должно пройти без ошибки.
        upsert_object_type(&conn, &ot)
            .expect("upsert_object_type должен работать внутри внешней транзакции");
        conn.execute_batch("COMMIT").unwrap();
        // Проверяем, что тип записан.
        let types = list_object_types(&conn).unwrap();
        assert!(
            types.iter().any(|t| t.id == "ot-nested"),
            "object_type должен быть записан"
        );
    }

    #[test]
    fn compact_usage_span_is_idempotent_and_dictionary_encoded() {
        let conn = setup_db();
        let mut write = UsageSpanWrite {
            device_id: "device-compact".to_string(),
            started_at_unix: 1_767_225_600,
            ended_at_unix: 1_767_225_630,
            tracked_app_id: "browser".to_string(),
            window_title: Some("Facebook".to_string()),
            flags: 0,
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        upsert_usage_span(&conn, &write).unwrap();
        write.ended_at_unix += 30;
        upsert_usage_span(&conn, &write).unwrap();

        let day = load_usage_day(&conn, "usage-day:device-compact:2026-01-01")
            .unwrap()
            .unwrap();
        assert_eq!(day.payload_json["a"], json!(["browser"]));
        assert_eq!(day.payload_json["t"], json!(["Facebook"]));
        assert_eq!(day.payload_json["s"], json!([[0, 60, 0, 0, 0]]));
        assert_eq!(
            get_usage_title_total(&conn, "face").unwrap(),
            UsageTitleTotal {
                active_seconds: 60,
                idle_seconds: 0,
            }
        );
    }

    #[test]
    fn compact_usage_span_splits_at_utc_midnight_and_syncs() {
        let source = setup_db();
        let write = UsageSpanWrite {
            device_id: "device-midnight".to_string(),
            started_at_unix: 1_767_311_990,
            ended_at_unix: 1_767_312_010,
            tracked_app_id: "reader".to_string(),
            window_title: Some("Book".to_string()),
            flags: 1,
            updated_at: "2026-01-02T00:00:10.000Z".to_string(),
        };
        let days = upsert_usage_span(&source, &write).unwrap();
        assert_eq!(days.len(), 2);
        ensure_usage_sequence_migrated(&source, "source-device").unwrap();

        let entities = SqliteStorageBackend::collect_entities_blocking(
            &source,
            &VersionVector::new(),
            "source-device",
        );
        let usage_days = entities
            .iter()
            .filter(|entity| entity.entity_type == "usage_day")
            .collect::<Vec<_>>();
        assert_eq!(usage_days.len(), 2);

        let peer = setup_db();
        for entity in usage_days {
            SqliteStorageBackend::apply_entity_blocking(&peer, entity).unwrap();
        }
        assert_eq!(
            peer.query_row("SELECT COUNT(*) FROM usage_days", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn usage_sequence_cursor_waits_for_missing_entries() {
        let conn = setup_db();

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-2",
            "peer-device",
            2,
            "2026-01-02T00:00:00.000Z:000002:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"0".to_string()));

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-1",
            "peer-device",
            1,
            "2026-01-01T00:00:00.000Z:000001:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"2".to_string()));
    }

    #[test]
    fn local_usage_versions_use_one_device_cursor() {
        let conn = setup_db();
        let first =
            bump_sync_version_vector(&conn, "usage_day", "day-a", "local-device", false).unwrap();
        let second =
            bump_sync_version_vector(&conn, "usage_day", "day-b", "local-device", false).unwrap();

        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:local-device"), Some(&"2".to_string()));
        assert!(!vector.contains_key("day-a"));
        assert!(!vector.contains_key("day-b"));
        assert!(HLC::is_newer(&second, &first));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            2
        );
    }

    #[test]
    fn legacy_usage_replay_keeps_one_synthetic_sequence() {
        let conn = setup_db();
        let day = UsageDay {
            id: "usage-day:legacy:2026-01-01".to_string(),
            device_id: "legacy".to_string(),
            day: "2026-01-01".to_string(),
            payload_json: json!({"a": ["browser"], "t": ["Example"], "s": [[0, 30, 0, 0, 0]]}),
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        let mut data = serde_json::to_value(&day)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        data.remove("id");
        let entity = SyncEntity {
            entity_type: "usage_day".to_string(),
            id: day.id.clone(),
            data,
            hlc: "2026-01-01T00:00:30.000Z:000007:old-peer".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };

        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();
        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();

        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(
            vector.get("@usage:legacy:old-peer").map(String::as_str),
            Some("1")
        );
        assert!(!vector.contains_key(&day.id));
    }

    #[tokio::test]
    async fn storage_backend_versioned_object_matrix_holds_unknown_payload_and_replays_exactly() {
        let backend = make_backend();
        let conn = backend.conn.clone();
        {
            let guard = conn.lock().unwrap();
            upsert_object_type(&guard, &make_object_type("remote-type", "Remote")).unwrap();
        }
        let mut old = sync_object("remote-old", "remote-type", "old-wire");
        old.data.remove("typeVersion");
        backend.apply_entity(&old).await.unwrap();
        assert_eq!(
            get_object(&conn.lock().unwrap(), "remote-old")
                .unwrap()
                .unwrap()
                .type_version,
            type_registry::LEGACY_VERSION
        );
        let mut unknown = sync_object("remote-unknown", "remote-type", "unknown-wire");
        unknown.data.insert("typeVersion".into(), json!("9.9.9"));
        unknown
            .data
            .insert("futurePayload".into(), json!({"keep":true}));
        backend.apply_entity(&unknown).await.unwrap();
        let guard = conn.lock().unwrap();
        assert!(get_object(&guard, "remote-unknown").unwrap().is_none());
        let payload: String = guard
            .query_row(
                "SELECT payload FROM sync_pending_objects WHERE id='remote-unknown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(payload.contains("futurePayload"));
        drop(guard);
        let replayed = {
            let guard = conn.lock().unwrap();
            replay_pending_for_type(&guard, "remote-type", type_registry::LEGACY_VERSION).unwrap()
        };
        assert_eq!(replayed, 0);
        assert!(get_object(&conn.lock().unwrap(), "remote-unknown")
            .unwrap()
            .is_none());
    }
