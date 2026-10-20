use super::*;
use crate::replication_storage::AuthenticatedEntityBackend;

#[path = "object_revision_compat/task_merge.rs"]
mod task_merge;

async fn admit_inbound_peer(state: &Arc<ServiceState>) {
    let handle = get_db_handle(state).unwrap();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
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
) -> ark_application::sync_storage::InboundApplyOutcome {
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
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let storage = ark_store::test_support::ObjectRevisionFixture::open(&fixture.db_path()).unwrap();
    let vector_object = storage
        .seed_legacy_object(
            task("legacy-vector-task", "vector wins"),
            ark_store::test_support::LegacyRevisionSeed::VersionVector {
                hlc: "2999-01-01T00:00:00.000Z:000001:legacy".into(),
            },
        )
        .unwrap();
    let tombstone_object = storage
        .seed_legacy_object(
            task("legacy-tombstone-task", "deleted"),
            ark_store::test_support::LegacyRevisionSeed::Tombstone {
                hlc: "2999-01-01T00:00:00.000Z:000002:legacy".into(),
                type_id: None,
            },
        )
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
        handle_request(
            &state,
            Request::GetObject {
                id: vector_object.id.clone(),
            },
        )
        .await
        .unwrap()["title"],
        "vector wins"
    );
    assert!(
        !serde_json::from_value::<ObjectWriteSnapshot>(
            handle_request(
                &state,
                Request::GetObjectWriteSnapshot {
                    id: tombstone_object.id,
                },
            )
            .await
            .unwrap(),
        )
        .unwrap()
        .exists
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
    ark_store::test_support::ObjectRevisionFixture::open(&fixture.db_path())
        .unwrap()
        .set_revision_hlc(
            "max-counter-task",
            "2999-01-01T00:00:00.000Z:18446744073709551615:remote",
        )
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
        handle_request(
            &state,
            Request::GetObject {
                id: "max-counter-task".into(),
            },
        )
        .await
        .unwrap()["title"],
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

    let storage = ark_store::test_support::ObjectRevisionFixture::open(&fixture.db_path()).unwrap();
    let object = storage
        .prepare_object(task("inbound-full-task", "remote"))
        .unwrap();
    let entity = full_task_entity(&object, "2990-01-01T00:00:00.000Z:000001:inbound-peer");
    assert_eq!(
        apply_authenticated(&state, &entity, "full-task-message").await,
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &entity, "full-task-message").await,
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );

    let mut stale = entity.clone();
    stale.hlc = "2989-01-01T00:00:00.000Z:000001:inbound-peer".into();
    stale.data.insert("title".into(), json!("stale"));
    assert_eq!(
        apply_authenticated(&state, &stale, "stale-task-message").await,
        ark_application::sync_storage::InboundApplyOutcome::Stale
    );

    let stored = storage.inspect(&object.id).unwrap();
    assert_eq!(
        handle_request(
            &state,
            Request::GetObject {
                id: object.id.clone()
            }
        )
        .await
        .unwrap()["title"],
        "remote"
    );
    assert_eq!(
        stored.receipt_count, 1,
        "replay must not duplicate the full receipt"
    );
    assert_eq!(stored.task_operation_count, 0);
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

    let storage = ark_store::test_support::ObjectRevisionFixture::open(&fixture.db_path()).unwrap();
    storage
        .seed_object(crate::types::ArkObjectWrite {
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
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &legacy, "legacy-task-message").await,
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );
    let legacy_state = storage.inspect(&legacy.id).unwrap();
    assert_eq!(legacy_state.object.as_ref().unwrap().type_id, "core.task");
    assert_eq!(
        handle_request(
            &state,
            Request::GetObject {
                id: legacy.id.clone()
            }
        )
        .await
        .unwrap()["title"],
        "legacy title"
    );
    assert_eq!(
        legacy_state.link_count, 1,
        "legacy project relation must map to a link"
    );
    assert_eq!(
        legacy_state.receipt_count, 0,
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
        ark_application::sync_storage::InboundApplyOutcome::Applied
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
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );
    assert_eq!(
        apply_authenticated(&state, &tombstone, "absent-task-delete").await,
        ark_application::sync_storage::InboundApplyOutcome::Applied
    );
    let tombstone_state = storage.inspect(&tombstone.id).unwrap();
    let legacy_tombstone_state = storage.inspect(&legacy_tombstone.id).unwrap();
    assert!(tombstone_state.object.is_none());
    assert!(legacy_tombstone_state.object.is_none());
    assert_eq!(tombstone_state.tombstone.unwrap().hlc, tombstone.hlc);
    assert_eq!(tombstone_state.task_operation_count, 0);
    assert_eq!(legacy_tombstone_state.task_operation_count, 0);
    let persisted_tombstone = legacy_tombstone_state.tombstone.unwrap();
    assert_eq!(persisted_tombstone.hlc, legacy_tombstone.hlc);
    assert!(
        persisted_tombstone.type_id.is_none(),
        "legacy Todo tombstones keep no type hint"
    );
}
