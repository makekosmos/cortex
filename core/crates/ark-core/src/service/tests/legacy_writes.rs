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

    let mut canonical_task = canonical_task_object(Some("0.1.0"), canonical_task_props());
    canonical_task.id = "todo-canonical-cutover".into();
    handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task,
            expected_snapshot: None,
            operation_id: Some("kos423-canonical-upsert".into()),
            device_id: Some("device-canonical".into()),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::DeleteObject {
            id: "todo-canonical-cutover".into(),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("device-canonical".into()),
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

    let fixture = ark_store::test_support::LegacyWriteFixture::open(&db_path).unwrap();
    let snapshot = fixture.inspect_successful_writes().unwrap();
    for id in [
        "project-legacy",
        "tag-legacy",
        "todo-legacy",
        "todo-batch-1",
        "todo-batch-2",
        "tracked-app-legacy",
    ] {
        assert!(
            snapshot
                .version_vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-legacy")),
            "{id} should have a local HLC in the version vector after legacy upsert",
        );
    }
    assert!(snapshot
        .version_vector
        .get("todo-canonical-cutover")
        .is_some_and(|hlc| hlc.ends_with(":device-canonical")));
    assert!(snapshot
        .version_vector
        .get(&snapshot.task_link_id)
        .is_some_and(|hlc| hlc.ends_with(":device-legacy")));
    assert!(
        snapshot.todo_tombstone_exists,
        "DeleteTodo writes a tombstone"
    );
    assert!(
        snapshot.tracked_app_tombstone_exists,
        "tracked-app deletion is synced"
    );
    assert!(
        snapshot.legacy_todo_deleted_at.is_some(),
        "DeleteTodo keeps a soft-deleted task row"
    );
    assert!(
        !snapshot.canonical_object_exists,
        "canonical DeleteObject hard-deletes the task row"
    );
    assert_eq!(
        snapshot.canonical_applied_receipt_count, 1,
        "canonical task upsert keeps its entity receipt"
    );
    assert_eq!(
        snapshot.committed_task_feed_count, 6,
        "task writes and deletes stay in committed feed"
    );
    assert_eq!(
        snapshot.task_merge_bootstrap_count, 4,
        "each task enters causal sync in the same transaction as its write"
    );
    assert_eq!(
        snapshot.task_existence_operation_count, 6,
        "each baseline records existence and both deletion modes append a causal tombstone"
    );
    fixture.install_todo_batch_failure().unwrap();
    let failed = handle_request(
        &state,
        Request::BatchUpsertTodos {
            todos: vec![make_todo("todo-fail-1"), make_todo("todo-fail-2")],
            device_id: device.clone(),
        },
    )
    .await;
    assert!(failed.is_err());
    for id in ["todo-fail-1", "todo-fail-2"] {
        let state = fixture.inspect_batch_todo(id).unwrap();
        assert_eq!(
            state,
            ark_store::test_support::LegacyBatchRollbackState {
                object_exists: false,
                revision_exists: false,
                links_exist: false,
            },
            "failed batch must roll back every write for {id}"
        );
    }
    fixture.install_project_delete_failure().unwrap();
    let failed_delete = handle_request(
        &state,
        Request::DeleteProject {
            id: "project-legacy".to_string(),
            device_id: device,
        },
    )
    .await;
    assert!(failed_delete.is_err());
    assert_eq!(
        fixture.inspect_project_delete().unwrap(),
        ark_store::test_support::LegacyProjectDeleteState {
            deleted_at: None,
            tombstone_exists: false,
        },
        "failed delete must restore the object and leave no tombstone"
    );
}

// -----------------------------------------------------------------------
// RED-тесты: локальная запись должна рассылать LiveChange пирам
// -----------------------------------------------------------------------

/// Fake SyncTransport: захватывает все отправленные LanSyncMessage в shared buf.
pub(super) struct CapturingTransport {
    pub(super) sent: Arc<TokioMutex<Vec<ark_sync_contracts::wire::LanSyncMessage>>>,
}

#[async_trait::async_trait]
impl crate::sync_transport::SyncTransport for CapturingTransport {
    async fn start(
        &self,
        _event_tx: tokio::sync::mpsc::UnboundedSender<crate::sync_transport::TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, msg: ark_sync_contracts::wire::LanSyncMessage) -> Result<(), String> {
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

#[async_trait::async_trait]
impl crate::sync_transport::IrohSessionTransport for CapturingTransport {
    async fn send_to(
        &self,
        _: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<(), String> {
        crate::sync_transport::SyncTransport::send(self, message)
    }
    fn set_outbound_authorizer(
        &self,
        _: Arc<dyn ark_sync_contracts::transport::OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn bind_peer_for_control(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn unbind_peer(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    async fn send_to_transport_peer(
        &self,
        _: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<crate::sync_transport::AddressedSendOutcome, String> {
        crate::sync_transport::SyncTransport::send(self, message)?;
        Ok(crate::sync_transport::AddressedSendOutcome::Sent)
    }
    fn disconnect_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
    fn disconnect_transport_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

#[async_trait::async_trait]
impl ark_sync_contracts::peer_control::PeerSessionControl for CapturingTransport {
    async fn send_control_to_endpoint(
        &self,
        _: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<ark_sync_contracts::peer_control::PeerControlSendOutcome, String> {
        crate::sync_transport::SyncTransport::send(self, message)?;
        Ok(ark_sync_contracts::peer_control::PeerControlSendOutcome::Sent)
    }
    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn unbind_peer(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn disconnect_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
    fn disconnect_transport_endpoint(
        &self,
        _: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
}
