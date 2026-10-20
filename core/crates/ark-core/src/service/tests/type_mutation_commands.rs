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

    let count = with_conn(&state, |connection| {
        connection
            .query_row(
                "SELECT COUNT(*) FROM object_types WHERE id = 'test.package.first'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(
        count, 0,
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
    with_conn(&state, |connection| {
        ark_store::pending_objects::insert_pending_object(connection, &pending, TYPE_ID)
    })
    .unwrap();

    with_conn(&state, |connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER fail_type_revision_insert BEFORE INSERT ON sync_kv \
                 WHEN NEW.key = 'lan_sync.version_vector' BEGIN \
                 SELECT RAISE(ABORT, 'injected type revision failure'); END; \
                 CREATE TRIGGER fail_type_revision_update BEFORE UPDATE ON sync_kv \
                 WHEN OLD.key = 'lan_sync.version_vector' BEGIN \
                 SELECT RAISE(ABORT, 'injected type revision failure'); END;",
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();

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

    let state_after_failure = with_conn(&state, |connection| {
        let type_count = connection
            .query_row(
                "SELECT COUNT(*) FROM object_types WHERE id = ?1",
                [TYPE_ID],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let pending_count = ark_store::pending_objects::count_for_type(connection, TYPE_ID)?;
        let object_count = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [ENTITY_ID],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((type_count, pending_count, object_count))
    })
    .unwrap();
    assert_eq!(state_after_failure, (0, 1, 0));

    with_conn(&state, |connection| {
        connection
            .execute_batch(
                "DROP TRIGGER fail_type_revision_insert; \
                 DROP TRIGGER fail_type_revision_update;",
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();
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
    let final_counts = with_conn(&state, |connection| {
        let pending_count = ark_store::pending_objects::count_for_type(connection, TYPE_ID)?;
        let object_count = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [ENTITY_ID],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((pending_count, object_count))
    })
    .unwrap();
    assert_eq!(final_counts, (0, 1));
}
