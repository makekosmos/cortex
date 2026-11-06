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
        .call("upsert_object", note("atlas-source", "Source"))
        .await
        .expect("upsert source");
    service
        .call("upsert_object", note("atlas-target", "Target"))
        .await
        .expect("upsert target");

    // 1. get_object
    let source = service
        .call("get_object", json!({"id": "atlas-source"}))
        .await
        .expect("get_object");
    assert_eq!(source["title"], "Source");

    // 2. get_object_write_snapshot
    let snapshot = service
        .call("get_object_write_snapshot", json!({"id": "atlas-source"}))
        .await
        .expect("get_object_write_snapshot");
    assert!(snapshot.get("revision").is_some());

    // 3. list_objects_by_type
    let notes = service
        .call("list_objects_by_type", json!({"type_id": "core.note"}))
        .await
        .expect("list_objects_by_type");
    assert_eq!(notes.as_array().map(Vec::len), Some(2));

    // 4. upsert_object (update the source title)
    let updated = note("atlas-source", "Source updated");
    service
        .call("upsert_object", updated)
        .await
        .expect("upsert_object update");

    // 5. replace_object_link
    service
        .call(
            "replace_object_link",
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
        .expect("replace_object_link");

    // 6. list_object_links
    let links = service
        .call("list_object_links", json!({}))
        .await
        .expect("list_object_links");
    let links = links.as_array().expect("links array");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["id"], "atlas-link");

    // 7. delete_object_link
    service
        .call("delete_object_link", json!({"id": "atlas-link"}))
        .await
        .expect("delete_object_link");

    // 8. delete_object
    service
        .call("delete_object", json!({"id": "atlas-target"}))
        .await
        .expect("delete_object");
    let remaining = service
        .call("list_objects_by_type", json!({"type_id": "core.note"}))
        .await
        .expect("list after delete");
    assert_eq!(remaining.as_array().map(Vec::len), Some(1));

    // 9-10. appearance.get / appearance.set through their runtime handler.
    let appearance = AppearanceStore::new(directory.path().join("appearance"));
    let manager = DispatchClient {
        class: Some("manager-gpui".into()),
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
