use engine::appearance::AppearanceStore;
use engine::engine_dispatch::DispatchClient;
use kernel::service::ArkService;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn note(id: &str, title: &str) -> Value {
    json!({
        "object": {
            "id": id,
            "typeId": "core.note",
            "typeVersion": "0.1.0",
            "title": title,
            "contentJson": {"type":"doc","content":[]},
            "propsJson": {"description": null, "extensions": {}},
            "createdAt": "2026-01-01T00:00:00.000Z",
            "updatedAt": "2026-01-01T00:00:00.000Z",
            "deletedAt": null
        }
    })
}

#[tokio::test]
async fn atlas_ten_operations_through_ark_service_and_appearance_handler() {
    let directory = tempfile::tempdir().expect("atlas ops temp dir");
    let db_path = directory.path().join("ark.db");
    let service = ArkService::open(db_path.to_string_lossy().into_owned())
        .await
        .expect("service opens");

    // Setup: two note objects so a real link can be created between them.
    service
        .call("objects.upsert", note("atlas-source", "Source"))
        .await
        .expect("upsert source");
    service
        .call("objects.upsert", note("atlas-target", "Target"))
        .await
        .expect("upsert target");

    // 1. get_object
    let source = service
        .call("objects.get", json!({"id": "atlas-source"}))
        .await
        .expect("objects.get");
    assert_eq!(source["title"], "Source");

    // 2. get_object_write_snapshot
    let snapshot = service
        .call("objects.snapshot", json!({"id": "atlas-source"}))
        .await
        .expect("objects.snapshot");
    assert!(snapshot.get("revision").is_some());

    // 3. list_objects_by_type
    let notes = service
        .call("objects.list", json!({"typeId": "core.note"}))
        .await
        .expect("objects.list");
    assert_eq!(notes.as_array().map(Vec::len), Some(2));

    // 4. upsert_object (update the source title)
    let updated = note("atlas-source", "Source updated");
    service
        .call("objects.upsert", updated)
        .await
        .expect("upsert_object update");

    // 5. replace_object_link
    service
        .call(
            "links.replace",
            json!({
                "object_link": {
                    "id": "atlas-link",
                    "sourceObjectId": "atlas-source",
                    "targetObjectId": "atlas-target",
                    "linkType": "related",
                    "createdAt": "2026-01-01T00:00:00.000Z"
                }
            }),
        )
        .await
        .expect("links.replace");

    // 6. list_object_links
    let links = service
        .call("links.list", json!({}))
        .await
        .expect("links.list");
    let links = links.as_array().expect("links array");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["id"], "atlas-link");

    // 7. delete_object_link
    service
        .call("links.delete", json!({"id": "atlas-link"}))
        .await
        .expect("links.delete");

    // 8. delete_object
    service
        .call("objects.delete", json!({"id": "atlas-target"}))
        .await
        .expect("objects.delete");
    let remaining = service
        .call("objects.list", json!({"typeId": "core.note"}))
        .await
        .expect("list after delete");
    assert_eq!(remaining.as_array().map(Vec::len), Some(1));

    // 9-10. appearance.get / appearance.set through their runtime handler.
    let appearance = AppearanceStore::new(directory.path().join("appearance"));
    let manager = DispatchClient {
        class: Some("manager".into()),
        ..DispatchClient::default()
    };
    let before = appearance
        .get(&json!({}), &manager)
        .await
        .expect("appearance.get");
    assert!(before["settings"].get("dark_theme").is_some());
    let changed = appearance
        .set(&json!({"dark_theme": "claude"}), &manager)
        .await
        .expect("appearance.set");
    assert_eq!(changed["settings"]["dark_theme"], "claude");

    service
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("service shuts down");
}
