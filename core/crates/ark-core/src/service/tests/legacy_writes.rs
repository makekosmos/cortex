use super::*;

#[tokio::test]
async fn legacy_entity_writes_bump_version_vector() {
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
    handle_request(
        &state,
        Request::UpsertProject {
            project,
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();

    let tag = Tag {
        id: "tag-legacy".to_string(),
        title: "Tag".to_string(),
        color: None,
        created_at: timestamp.clone(),
    };
    handle_request(
        &state,
        Request::UpsertTag {
            tag,
            device_id: device.clone(),
        },
    )
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

    handle_request(
        &state,
        Request::UpsertTodo {
            todo: make_todo("todo-legacy"),
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();

    // Batch upsert тоже должен bump'ать version vector per-entity.
    handle_request(
        &state,
        Request::BatchUpsertTodos {
            todos: vec![make_todo("todo-batch-1"), make_todo("todo-batch-2")],
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();

    // Delete legacy — должен записать tombstone.
    handle_request(
        &state,
        Request::DeleteTodo {
            id: "todo-batch-1".to_string(),
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();

    handle_request(
        &state,
        Request::UpsertTrackedApp {
            tracked_app: TrackedApp {
                id: "tracked-app-legacy".into(),
                platform: "test".into(),
                exe_path: "/tmp/app".into(),
                normalized_exe_path: "/tmp/app".into(),
                process_name: "app".into(),
                display_name: Some("App".into()),
                publisher: None,
                icon_ref: None,
                first_seen_at: timestamp.clone(),
                last_seen_at: timestamp.clone(),
            },
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::DeleteTrackedApp {
            id: "tracked-app-legacy".into(),
            device_id: device.clone(),
        },
    )
    .await
    .unwrap();

    let shared = get_db_handle(&state)
        .map(|db| db.shared_connection_for_test())
        .unwrap();
    {
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
            "tracked-app-legacy",
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
        let tracked_app_tombstone: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["tracked-app-legacy", "tracked_app"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tracked_app_tombstone, 1);

        guard
            .execute_batch(
                "CREATE TRIGGER fail_legacy_batch BEFORE INSERT ON object_sync_versions
             WHEN NEW.object_id = 'todo-fail-2'
             BEGIN SELECT RAISE(ABORT, 'injected batch failure'); END;",
            )
            .unwrap();
    }
    let failed = handle_request(
        &state,
        Request::BatchUpsertTodos {
            todos: vec![make_todo("todo-fail-1"), make_todo("todo-fail-2")],
            device_id: device.clone(),
        },
    )
    .await;
    assert!(failed.is_err());
    {
        let guard = shared.lock().unwrap();
        for id in ["todo-fail-1", "todo-fail-2"] {
            let object_count: i64 = guard
                .query_row("SELECT COUNT(*) FROM objects WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .unwrap();
            let version_count: i64 = guard
                .query_row(
                    "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(object_count, 0, "failed batch must roll back object {id}");
            assert_eq!(version_count, 0, "failed batch must roll back version {id}");
        }
    }
}

// -----------------------------------------------------------------------
// RED-тесты: локальная запись должна рассылать LiveChange пирам
// -----------------------------------------------------------------------

/// Fake SyncTransport: захватывает все отправленные LanSyncMessage в shared buf.
pub(super) struct CapturingTransport {
    pub(super) sent: Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>>,
}

#[async_trait::async_trait]
impl crate::sync_transport::SyncTransport for CapturingTransport {
    fn supports_registry_admission(&self) -> bool {
        true
    }

    async fn start(
        &self,
        _event_tx: tokio::sync::mpsc::UnboundedSender<crate::sync_transport::TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, msg: crate::protocol::LanSyncMessage) -> Result<(), String> {
        // `send` — sync, но нам нужен lock на TokioMutex из sync контекста.
        // Используем blocking_lock через spawn_blocking или try_lock; в тестах
        // конкурентности нет, try_lock гарантированно успевает.
        self.sent
            .try_lock()
            .expect("CapturingTransport: lock")
            .push(msg);
        Ok(())
    }

    async fn stop(&self) {}
}
