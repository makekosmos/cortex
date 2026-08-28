
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
