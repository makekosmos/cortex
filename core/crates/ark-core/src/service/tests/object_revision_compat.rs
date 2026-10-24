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
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
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
    initialize_test_database(
        &state,
        dir.path().join("ark.db").to_string_lossy().to_string(),
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
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
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
