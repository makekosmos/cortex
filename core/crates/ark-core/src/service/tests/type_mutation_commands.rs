use super::*;
use serde_json::json;

fn package_registration(type_id: &str) -> crate::type_registry::TypeRegistration {
    let mut registration = crate::canonical_types::definitions::canonical_type_registrations()
        .expect("canonical registrations should load")
        .remove(0);
    registration.type_id = type_id.to_string();
    registration.name = format!("Test package type {type_id}");
    registration.aliases.clear();
    registration
}

fn replay_event_for(
    receiver: &mut tokio::sync::broadcast::Receiver<Value>,
    entity_id: &str,
) -> Vec<Value> {
    let mut events = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(event) if event["event"] == "sync_replay" && event["entity_id"] == entity_id => {
                events.push(event);
            }
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            | Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
    events
}

#[tokio::test]
async fn package_type_registration_rolls_back_the_whole_batch() {
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

    let first = package_registration("test.package.first");
    let mut invalid = package_registration("test.package.invalid");
    invalid.status = "not-a-status".into();
    let result = handle_request(
        &state,
        Request::TypesRegisterPackageDefinitions {
            registrations: vec![first, invalid],
        },
    )
    .await;
    assert!(result.is_err(), "invalid package definition must fail");

    assert!(
        handle_request(
            &state,
            Request::GetObjectType {
                id: "test.package.first".into(),
            },
        )
        .await
        .unwrap()
        .is_null(),
        "earlier registrations must roll back with the batch"
    );
}

#[tokio::test]
async fn replay_notice_is_published_only_after_the_local_type_write_commits() {
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

    const TYPE_ID: &str = "test.replay.type";
    const ENTITY_ID: &str = "test.replay.object";
    let mut data = serde_json::Map::new();
    data.insert("typeId".into(), json!(TYPE_ID));
    data.insert("typeVersion".into(), json!("0.0.0-legacy"));
    data.insert("title".into(), json!("Pending object"));
    data.insert("contentJson".into(), json!({}));
    data.insert("propsJson".into(), json!({}));
    data.insert("createdAt".into(), json!("2026-10-01T00:00:00.000Z"));
    data.insert("updatedAt".into(), json!("2026-10-01T00:00:00.000Z"));
    let pending = crate::types::SyncEntity {
        entity_type: "object".into(),
        id: ENTITY_ID.into(),
        data,
        hlc: "2026-10-01T00:00:00.000Z:000001:remote-device".into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let type_fixture =
        ark_store::test_support::TypeMutationFixture::open(&fixture.db_path()).unwrap();
    type_fixture
        .insert_pending_object(&pending, TYPE_ID)
        .unwrap();
    type_fixture.reject_sync_revision_writes().unwrap();

    let mut events = crate::events::subscribe();
    let object_type = crate::types::ObjectType {
        id: TYPE_ID.into(),
        name: "Replay test type".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "2026-10-01T00:00:00.000Z".into(),
        updated_at: "2026-10-01T00:00:00.000Z".into(),
        system_locked: false,
    };
    let failed = handle_request(
        &state,
        Request::UpsertObjectType {
            object_type: object_type.clone(),
            device_id: Some("local-device".into()),
        },
    )
    .await;
    assert!(failed.is_err(), "the injected revision write must fail");
    assert!(
        replay_event_for(&mut events, ENTITY_ID).is_empty(),
        "a replay notice must not escape a rolled-back transaction"
    );

    assert_eq!(
        type_fixture.inspect(TYPE_ID, ENTITY_ID).unwrap(),
        ark_store::test_support::TypeMutationState {
            registered: false,
            pending_objects: 1,
            object_exists: false,
        }
    );

    type_fixture.allow_sync_revision_writes().unwrap();
    handle_request(
        &state,
        Request::UpsertObjectType {
            object_type,
            device_id: Some("local-device".into()),
        },
    )
    .await
    .unwrap();

    let replay_events = replay_event_for(&mut events, ENTITY_ID);
    assert_eq!(replay_events.len(), 1);
    assert_eq!(
        replay_events[0],
        json!({
            "event": "sync_replay",
            "entity_type": "object",
            "entity_id": ENTITY_ID,
            "type_id": TYPE_ID,
        })
    );
    assert_eq!(
        type_fixture.inspect(TYPE_ID, ENTITY_ID).unwrap(),
        ark_store::test_support::TypeMutationState {
            registered: true,
            pending_objects: 0,
            object_exists: true,
        }
    );
}
