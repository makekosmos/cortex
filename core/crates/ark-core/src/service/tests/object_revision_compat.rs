use super::*;
use crate::replication_storage::AuthenticatedEntityBackend;

#[path = "object_revision_compat/task_merge.rs"]
mod task_merge;

async fn admit_inbound_peer(state: &Arc<ServiceState>) {
    let handle = get_db_handle(state).unwrap();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            db::CommandContext::engine(),
            db::EnrollPairedDeviceCommand::new(
                "ark-core-rpc-local".into(),
                "inbound-peer".into(),
                endpoint,
            ),
        )
        .await
        .unwrap();
}

async fn apply_authenticated(
    state: &Arc<ServiceState>,
    entity: &SyncEntity,
    message_id: &str,
) -> crate::application::sync_storage::InboundApplyOutcome {
    SqliteStorageBackend::with_db_handle(get_db_handle(state).unwrap())
        .apply_authenticated_entity_with_id(entity, "inbound-peer", message_id)
        .await
        .unwrap()
}

fn full_task_entity(object: &ArkObject, hlc: &str) -> SyncEntity {
    let Value::Object(mut data) = serde_json::to_value(object).unwrap() else {
        panic!("canonical object must serialize as a map")
    };
    data.remove("id");
    SyncEntity {
        entity_type: "object".into(),
        id: object.id.clone(),
        data,
        hlc: hlc.into(),
        deleted: None,
        origin_device_id: Some("inbound-peer".into()),
        origin_seq: None,
    }
}

fn task(id: &str, title: &str) -> ArkObjectWrite {
    let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
    object.id = id.to_string();
    object.title = title.to_string();
    object
}

fn remote_entity(object: &ArkObject, title: &str) -> SyncEntity {
    let value = serde_json::to_value(object).unwrap();
    let Value::Object(mut data) = value else {
        panic!("expected JSON object, got {value:?}")
    };
    data.remove("id");
    data.insert("title".to_string(), json!(title));
    SyncEntity {
        entity_type: "object".to_string(),
        id: object.id.clone(),
        data,
        hlc: "2026-01-01T00:00:00.000Z:000001:remote".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

#[tokio::test]
async fn pre_version_table_revisions_reject_stale_remote_resurrection() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    handle_request(
        &state,
        Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let (vector_object, tombstone_object) = with_write_tx(&state, |conn| {
        let vector_object = crate::canonical_types::ingress::prepare_object(
            conn,
            task("legacy-vector-task", "vector wins"),
        )
        .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &vector_object)?;
        db::set_sync_kv(
            conn,
            "lan_sync.version_vector",
            r#"{"legacy-vector-task":"2999-01-01T00:00:00.000Z:000001:legacy"}"#,
        )?;

        let tombstone_object = crate::canonical_types::ingress::prepare_object(
            conn,
            task("legacy-tombstone-task", "deleted"),
        )
        .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &tombstone_object)?;
        db::delete_object(conn, &tombstone_object.id)?;
        db::record_sync_tombstone(
            conn,
            "object",
            &tombstone_object.id,
            "2999-01-01T00:00:00.000Z:000002:legacy",
        )?;
        Ok((vector_object, tombstone_object))
    })
    .unwrap();

    let backend = SqliteStorageBackend::with_db_handle(get_db_handle(&state).unwrap());
    backend
        .apply_entity(&remote_entity(&vector_object, "stale vector overwrite"))
        .await
        .unwrap();
    backend
        .apply_entity(&remote_entity(&tombstone_object, "stale resurrection"))
        .await
        .unwrap();

    assert_eq!(
        with_conn(&state, |conn| db::get_object(conn, &vector_object.id))
            .unwrap()
            .unwrap()
            .title,
        "vector wins"
    );
    assert!(
        with_conn(&state, |conn| db::get_object(conn, &tombstone_object.id))
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn max_remote_hlc_counter_rolls_back_local_write() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    handle_request(
        &state,
        Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state,
        Request::UpsertObject {
            object: task("max-counter-task", "original"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("local-writer".to_string()),
        },
    )
    .await
    .unwrap();
    with_write_tx(&state, |conn| {
        conn.execute(
            "UPDATE object_sync_versions SET hlc = ?2 WHERE object_id = ?1",
            rusqlite::params![
                "max-counter-task",
                "2999-01-01T00:00:00.000Z:18446744073709551615:remote"
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();
    let snapshot: ObjectWriteSnapshot = serde_json::from_value(
        handle_request(
            &state,
            Request::GetObjectWriteSnapshot {
                id: "max-counter-task".to_string(),
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        handle_request(
            &state,
            Request::UpsertObject {
                object: task("max-counter-task", "must roll back"),
                expected_snapshot: Some(snapshot),
                operation_id: None,
                device_id: Some("local-writer".to_string()),
            }
        )
        .await
        .unwrap_err(),
        "hlc_counter_overflow"
    );
    assert_eq!(
        with_conn(&state, |conn| db::get_object(conn, "max-counter-task"))
            .unwrap()
            .unwrap()
            .title,
        "original"
    );
}

#[tokio::test]
async fn authenticated_task_snapshot_preserves_receipt_stale_and_inbox_replay() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();
    admit_inbound_peer(&state).await;

    let object = with_conn(&state, |conn| {
        crate::canonical_types::ingress::prepare_object(conn, task("inbound-full-task", "remote"))
            .map_err(|error| error.to_string())
    })
    .unwrap();
    let entity = full_task_entity(&object, "2990-01-01T00:00:00.000Z:000001:inbound-peer");
    assert_eq!(
        apply_authenticated(&state, &entity, "full-task-message").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &entity, "full-task-message").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );

    let mut stale = entity.clone();
    stale.hlc = "2989-01-01T00:00:00.000Z:000001:inbound-peer".into();
    stale.data.insert("title".into(), json!("stale"));
    assert_eq!(
        apply_authenticated(&state, &stale, "stale-task-message").await,
        crate::application::sync_storage::InboundApplyOutcome::Stale
    );

    let (stored_title, receipt_count, operation_count) = with_conn(&state, |conn| {
        let title = db::get_object(conn, &object.id)?.unwrap().title;
        let receipts = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&object.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let operations = conn
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())?;
        Ok((title, receipts, operations))
    })
    .unwrap();
    assert_eq!(stored_title, "remote");
    assert_eq!(
        receipt_count, 1,
        "replay must not duplicate the full receipt"
    );
    assert_eq!(operation_count, 0);
}

#[tokio::test]
async fn authenticated_legacy_todo_mapping_and_absent_task_tombstone_use_task_seam() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();
    admit_inbound_peer(&state).await;

    with_write_tx(&state, |conn| {
        let project = crate::types::ArkObjectWrite {
            id: "legacy-project".into(),
            type_id: "com.kosmos.project".into(),
            type_version: Some("1.0.0".into()),
            title: "Legacy project target".into(),
            content_json: json!({
                "type": "doc",
                "content": [{ "type": "paragraph" }]
            }),
            props_json: json!({
                "status": "active",
                "scheduledAt": null,
                "dueAt": null,
                "color": null,
                "extensions": {}
            }),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            deleted_at: None,
        };
        let project = crate::canonical_types::ingress::prepare_object(conn, project)
            .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &project)
    })
    .unwrap();

    let legacy = SyncEntity {
        entity_type: "todo".into(),
        id: "inbound-legacy-task".into(),
        data: serde_json::from_value(json!({
            "title": "legacy title",
            "created_at": "2026-01-01T00:00:00.000Z",
            "project_id": "legacy-project"
        }))
        .unwrap(),
        hlc: "2990-01-01T00:00:00.000Z:000002:inbound-peer".into(),
        deleted: None,
        origin_device_id: Some("inbound-peer".into()),
        origin_seq: None,
    };
    assert_eq!(
        apply_authenticated(&state, &legacy, "legacy-task-message").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &legacy, "legacy-task-message").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    let (legacy_object, link_count, receipt_count) = with_conn(&state, |conn| {
        let object = db::get_object(conn, &legacy.id)?.unwrap();
        let links = conn
            .query_row(
                "SELECT COUNT(*) FROM object_links
                 WHERE source_object_id = ?1 AND target_object_id = 'legacy-project'
                   AND link_type = 'project'",
                [&legacy.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let receipts = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&legacy.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((object, links, receipts))
    })
    .unwrap();
    assert_eq!(legacy_object.type_id, "core.task");
    assert_eq!(legacy_object.title, "legacy title");
    assert_eq!(link_count, 1, "legacy project relation must map to a link");
    assert_eq!(
        receipt_count, 0,
        "legacy snapshots keep their no-receipt contract"
    );

    let tombstone = SyncEntity {
        entity_type: "object".into(),
        id: "inbound-absent-task".into(),
        data: serde_json::from_value(json!({ "typeId": "core.task" })).unwrap(),
        hlc: "2990-01-01T00:00:00.000Z:000003:inbound-peer".into(),
        deleted: Some(true),
        origin_device_id: Some("inbound-peer".into()),
        origin_seq: None,
    };
    assert_eq!(
        apply_authenticated(&state, &tombstone, "absent-task-delete").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    let legacy_tombstone = SyncEntity {
        entity_type: "todo".into(),
        id: "inbound-absent-legacy-task".into(),
        data: serde_json::Map::new(),
        hlc: "2990-01-01T00:00:00.000Z:000004:inbound-peer".into(),
        deleted: Some(true),
        origin_device_id: Some("inbound-peer".into()),
        origin_seq: None,
    };
    assert_eq!(
        apply_authenticated(&state, &legacy_tombstone, "absent-legacy-task-delete").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &tombstone, "absent-task-delete").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );
    let tombstone_state = with_conn(&state, |conn| {
        let object_count = conn
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [&tombstone.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let hlc = conn
            .query_row(
                "SELECT hlc FROM sync_tombstones WHERE id = ?1 AND entity_type = 'object'",
                [&tombstone.id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())?;
        let operations = conn
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())?;
        let legacy_hlc = conn
            .query_row(
                "SELECT hlc FROM sync_tombstones WHERE id = ?1 AND entity_type = 'object'",
                [&legacy_tombstone.id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())?;
        let legacy_type = conn
            .query_row(
                "SELECT type_id IS NULL FROM sync_tombstones WHERE id = ?1",
                [&legacy_tombstone.id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((object_count, hlc, legacy_hlc, legacy_type, operations))
    })
    .unwrap();
    let object_count = tombstone_state.0;
    let tombstone_hlc = tombstone_state.1;
    let legacy_tombstone_hlc = tombstone_state.2;
    let legacy_tombstone_type = tombstone_state.3;
    let operation_count = tombstone_state.4;
    assert_eq!(object_count, 0);
    assert_eq!(tombstone_hlc, tombstone.hlc);
    assert_eq!(legacy_tombstone_hlc, legacy_tombstone.hlc);
    assert!(
        legacy_tombstone_type,
        "legacy Todo tombstones keep no type hint"
    );
    assert_eq!(operation_count, 0);
}
