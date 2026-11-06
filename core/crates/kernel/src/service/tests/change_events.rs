//! Writes announce themselves on the event bus: `objects.changed` and
//! `links.changed` carry what a subscriber needs to refetch one thing.

use super::*;

fn drain_for(
    receiver: &mut tokio::sync::broadcast::Receiver<Value>,
    event: &str,
    id: &str,
) -> Option<Value> {
    loop {
        match receiver.try_recv() {
            Ok(value) if value["event"] == event && value["id"] == id => return Some(value),
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => return None,
            Err(error) => panic!("unexpected event receiver error: {error}"),
        }
    }
}

#[tokio::test]
async fn an_object_write_says_which_object_type_and_operation_changed() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let mut events = crate::events::subscribe();
    handle_request(&state, note_write("changed-note"))
        .await
        .unwrap();
    let changed = drain_for(&mut events, "objects.changed", "changed-note")
        .expect("objects.changed for changed-note");
    assert_eq!(changed["id"], "changed-note");
    assert_eq!(changed["typeId"], "core.note");
    assert_eq!(changed["op"], "upsert");
}

#[tokio::test]
async fn a_link_write_says_which_link_changed() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    for id in ["link-source", "link-target"] {
        handle_request(&state, note_write(id)).await.unwrap();
    }
    let mut events = crate::events::subscribe();
    handle_request(
        &state,
        Request::UpsertObjectLink {
            object_link: ObjectLink {
                id: "link-changed".into(),
                source_object_id: "link-source".into(),
                target_object_id: "link-target".into(),
                link_type: "related".into(),
                created_at: "2026-10-16T00:00:00.000Z".into(),
            },
            device_id: Some("link-device".into()),
        },
    )
    .await
    .unwrap();
    let changed = drain_for(&mut events, "links.changed", "link-changed")
        .expect("links.changed for link-changed");
    assert_eq!(changed["id"], "link-changed");
    assert_eq!(changed["sourceId"], "link-source");
    assert_eq!(changed["targetId"], "link-target");
    assert_eq!(changed["linkType"], "related");
    assert_eq!(changed["op"], "upsert");
}
