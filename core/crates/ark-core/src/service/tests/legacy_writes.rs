use super::*;

#[tokio::test]
async fn legacy_entity_writes_bump_version_vector() {
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

    let shared = get_shared_conn(&state).unwrap();
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

// -----------------------------------------------------------------------
// RED-тесты: локальная запись должна рассылать LiveChange пирам
// -----------------------------------------------------------------------

/// Fake SyncTransport: захватывает все отправленные LanSyncMessage в shared buf.
pub(super) struct CapturingTransport {
    pub(super) sent: Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>>,
}

#[async_trait::async_trait]
impl crate::sync_transport::SyncTransport for CapturingTransport {
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

    fn stop(&self) {}
}
