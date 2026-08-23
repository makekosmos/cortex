use super::*;

    #[tokio::test]
    async fn legacy_entity_writes_bump_version_vector() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let device = Some("device-legacy".to_string());
        let timestamp = "2026-05-18T00:00:00.000Z".to_string();

        let project = Project {
            id: "project-legacy".to_string(),
            title: "Project".to_string(),
            notes: None,
            status: "active".to_string(),
            scheduled_date: None,
            deadline: None,
            sort_order: 0,
            color_tag: None,
            area_id: Some("area-legacy".to_string()),
            created_at: timestamp.clone(),
        };
        handle_request(Request::UpsertProject {
            project,
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let tag = Tag {
            id: "tag-legacy".to_string(),
            title: "Tag".to_string(),
            color: None,
            created_at: timestamp.clone(),
        };
        handle_request(Request::UpsertTag {
            tag,
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let make_todo = |id: &str| TodoItem {
            id: id.to_string(),
            title: "Todo".to_string(),
            notes: None,
            priority: 0,
            scheduled_date: None,
            deadline: None,
            reminder_date: None,
            is_today: false,
            is_evening: false,
            is_someday: false,
            is_completed: true,
            completed_at: None,
            is_cancelled: false,
            cancelled_at: None,
            is_trashed: false,
            sort_order: 0,
            heading_id: None,
            project_id: Some("project-legacy".to_string()),
            area_id: None,
            tag_ids: vec![],
            checklist_items: json!([]),
            recurrence_rule: None,
            created_at: timestamp.clone(),
        };

        handle_request(Request::UpsertTodo {
            todo: make_todo("todo-legacy"),
            device_id: device.clone(),
        })
        .await
        .unwrap();

        // Batch upsert тоже должен bump'ать version vector per-entity.
        handle_request(Request::BatchUpsertTodos {
            todos: vec![make_todo("todo-batch-1"), make_todo("todo-batch-2")],
            device_id: device.clone(),
        })
        .await
        .unwrap();

        // Delete legacy — должен записать tombstone.
        handle_request(Request::DeleteTodo {
            id: "todo-batch-1".to_string(),
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let guard = shared.lock().unwrap();
        let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        for id in [
            "project-legacy",
            "tag-legacy",
            "todo-legacy",
            "todo-batch-1",
            "todo-batch-2",
        ] {
            assert!(
                vector
                    .get(id)
                    .is_some_and(|hlc| hlc.ends_with(":device-legacy")),
                "{id} should have a local HLC in the version vector after legacy upsert",
            );
        }

        let todo_tombstone: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["todo-batch-1", "object"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(todo_tombstone, 1, "DeleteTodo должен записать tombstone");
    }

    #[tokio::test]
    async fn legacy_planning_writes_are_read_only_without_sync_side_effects() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let snapshot = || {
            let shared = get_shared_conn().unwrap();
            let conn = shared.lock().unwrap();
            let vector = db::get_sync_kv(&conn, "lan_sync.version_vector").unwrap();
            let counts = [
                "areas",
                "headings",
                "todos",
                "projects",
                "tags",
                "objects",
                "object_links",
                "sync_tombstones",
                "sync_kv",
            ]
            .map(|table| {
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap()
            });
            (vector, counts)
        };

        let before = snapshot();
        let area = Area {
            id: "area-read-only".to_string(),
            title: "Area".to_string(),
            sort_order: 0,
            created_at: "2026-05-18T00:00:00.000Z".to_string(),
        };
        let heading = Heading {
            id: "heading-read-only".to_string(),
            title: "Heading".to_string(),
            sort_order: 0,
            project_id: "project-read-only".to_string(),
        };
        for request in [
            Request::UpsertArea {
                area,
                device_id: Some("device-read-only".to_string()),
            },
            Request::UpsertHeading {
                heading,
                device_id: Some("device-read-only".to_string()),
            },
            Request::DeleteHeading {
                id: "heading-read-only".to_string(),
                device_id: Some("device-read-only".to_string()),
            },
        ] {
            assert_eq!(
                handle_request(request).await.unwrap_err(),
                "LegacyPlanningReadOnly"
            );
        }
        assert_eq!(snapshot(), before);
    }

    // -----------------------------------------------------------------------
    // RED-тесты: локальная запись должна рассылать LiveChange пирам
    // -----------------------------------------------------------------------

    /// Fake SyncTransport: захватывает все отправленные LanSyncMessage в shared buf.
    pub(super) struct CapturingTransport {
        pub(super) sent: Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>>,
    }

    #[async_trait::async_trait]
    impl ark_core::sync_transport::SyncTransport for CapturingTransport {
        async fn start(
            &self,
            _event_tx: tokio::sync::mpsc::UnboundedSender<ark_core::sync_transport::TransportEvent>,
        ) -> Result<(), String> {
            Ok(())
        }

        fn send(&self, msg: ark_core::protocol::LanSyncMessage) -> Result<(), String> {
            // `send` — sync, но нам нужен lock на TokioMutex из sync контекста.
            // Используем blocking_lock через spawn_blocking или try_lock; в тестах
            // конкурентности нет, try_lock гарантированно успевает.
            self.sent
                .try_lock()
                .expect("CapturingTransport: lock")
                .push(msg);
            Ok(())
        }

        fn stop(&self) {}
    }
