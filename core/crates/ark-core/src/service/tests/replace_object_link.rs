use super::*;

const TIMESTAMP: &str = "2026-10-01T00:00:00.000Z";
const DEVICE: &str = "device-local";

fn device() -> Option<String> {
    Some(DEVICE.to_string())
}

fn project_link(id: &str, source: &str, target: &str) -> ObjectLink {
    ObjectLink {
        id: id.to_string(),
        source_object_id: source.to_string(),
        target_object_id: target.to_string(),
        link_type: "project".to_string(),
        created_at: TIMESTAMP.to_string(),
    }
}

/// Opens the DB and creates one task and two project objects.
async fn setup(state: &Arc<ServiceState>, db_path: String) {
    initialize_test_database(state, db_path).await.unwrap();
    let object_type = ObjectType {
        id: "replace-link-type".to_string(),
        name: "Replace link".to_string(),
        schema_json: "{}".to_string(),
        ui_schema_json: "{}".to_string(),
        created_at: TIMESTAMP.to_string(),
        updated_at: TIMESTAMP.to_string(),
        system_locked: false,
    };
    handle_request(
        state,
        Request::UpsertObjectType {
            object_type,
            device_id: device(),
        },
    )
    .await
    .unwrap();
    for id in ["task-1", "project-a", "project-b"] {
        let object = ArkObjectWrite {
            id: id.to_string(),
            type_id: "replace-link-type".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: id.to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: TIMESTAMP.to_string(),
            updated_at: TIMESTAMP.to_string(),
            deleted_at: None,
        };
        handle_request(
            state,
            Request::UpsertObject {
                object,
                expected_snapshot: None,
                operation_id: None,
                device_id: device(),
            },
        )
        .await
        .unwrap();
    }
}

/// All `project` links from `source`, as `list_object_links` returns them.
async fn project_links(state: &Arc<ServiceState>, source: &str) -> Vec<Value> {
    let links = handle_request(state, Request::ListObjectLinks)
        .await
        .unwrap();
    links
        .as_array()
        .unwrap()
        .iter()
        .filter(|link| link["sourceObjectId"] == source && link["linkType"] == "project")
        .cloned()
        .collect()
}

#[tokio::test]
async fn replace_object_link_moves_task_to_new_project_and_records_sync() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    setup(&state, fixture.db_path().to_string_lossy().to_string()).await;
    handle_request(
        &state,
        Request::UpsertObjectLink {
            object_link: project_link("link-a", "task-1", "project-a"),
            device_id: device(),
        },
    )
    .await
    .unwrap();

    let stored = handle_request(
        &state,
        Request::ReplaceObjectLink {
            object_link: project_link("link-b", "task-1", "project-b"),
            device_id: device(),
        },
    )
    .await
    .unwrap();
    assert_eq!(stored["id"], json!("link-b"));
    assert_eq!(stored["targetObjectId"], json!("project-b"));

    let links = project_links(&state, "task-1").await;
    assert_eq!(links.len(), 1, "exactly one project link: {links:?}");
    assert_eq!(links[0]["id"], json!("link-b"));
    assert_eq!(links[0]["targetObjectId"], json!("project-b"));

    let sync_state =
        ark_store::test_support::SyncPersistenceFixture::inspect(&fixture.db_path()).unwrap();
    assert!(
        sync_state
            .tombstones
            .iter()
            .any(|(id, entity_type)| id == "link-a" && entity_type == "object_link"),
        "removal of link-a must be recorded"
    );
    for id in ["link-a", "link-b"] {
        assert!(
            sync_state
                .version_vector
                .get(id)
                .is_some_and(|hlc| hlc.ends_with(":device-local")),
            "{id} should have a local HLC in the version vector",
        );
    }
}

#[tokio::test]
async fn replace_object_link_rejects_missing_target_and_changes_nothing() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    setup(&state, fixture.db_path().to_string_lossy().to_string()).await;
    handle_request(
        &state,
        Request::UpsertObjectLink {
            object_link: project_link("link-a", "task-1", "project-a"),
            device_id: device(),
        },
    )
    .await
    .unwrap();

    let result = handle_request(
        &state,
        Request::ReplaceObjectLink {
            object_link: project_link("link-missing", "task-1", "no-such-project"),
            device_id: device(),
        },
    )
    .await;
    assert!(result.is_err(), "missing target must be rejected");

    let links = project_links(&state, "task-1").await;
    assert_eq!(
        links.len(),
        1,
        "old link must survive the rollback: {links:?}"
    );
    assert_eq!(links[0]["id"], json!("link-a"));

    let sync_state =
        ark_store::test_support::SyncPersistenceFixture::inspect(&fixture.db_path()).unwrap();
    assert!(
        sync_state.tombstones.iter().all(|(id, _)| id != "link-a"),
        "rolled-back delete must leave no tombstone"
    );
}

#[tokio::test]
async fn replace_object_link_with_same_link_is_idempotent() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    setup(&state, fixture.db_path().to_string_lossy().to_string()).await;
    let link = project_link("link-b", "task-1", "project-b");
    for _ in 0..2 {
        handle_request(
            &state,
            Request::ReplaceObjectLink {
                object_link: link.clone(),
                device_id: device(),
            },
        )
        .await
        .unwrap();
    }

    let links = project_links(&state, "task-1").await;
    assert_eq!(links.len(), 1, "exactly one project link: {links:?}");
    assert_eq!(links[0]["id"], json!("link-b"));
    assert_eq!(links[0]["targetObjectId"], json!("project-b"));
}

#[tokio::test]
async fn replace_object_link_accepts_same_target_under_new_id() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    setup(&state, fixture.db_path().to_string_lossy().to_string()).await;
    handle_request(
        &state,
        Request::UpsertObjectLink {
            object_link: project_link("link-a", "task-1", "project-a"),
            device_id: device(),
        },
    )
    .await
    .unwrap();

    // Same (source, target, link_type) under a new id: the old row must go
    // before the new one is inserted, or UNIQUE(source, target, link_type)
    // rejects the upsert.
    handle_request(
        &state,
        Request::ReplaceObjectLink {
            object_link: project_link("link-a2", "task-1", "project-a"),
            device_id: device(),
        },
    )
    .await
    .unwrap();

    let links = project_links(&state, "task-1").await;
    assert_eq!(links.len(), 1, "exactly one project link: {links:?}");
    assert_eq!(links[0]["id"], json!("link-a2"));
}
