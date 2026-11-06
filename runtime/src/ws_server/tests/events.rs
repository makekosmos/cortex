//! Object writes reach WebSocket subscribers as `objects.changed`.

use super::*;

fn note_write(id: &str) -> serde_json::Value {
    serde_json::json!({
        "object": {
            "id": id,
            "typeId": "core.note",
            "typeVersion": "0.1.0",
            "title": "ws note",
            "contentJson": {"type": "doc", "content": []},
            "propsJson": {"description": null, "extensions": {}},
            "createdAt": "2026-10-16T00:00:00.000Z",
            "updatedAt": "2026-10-16T00:00:00.000Z",
            "deletedAt": null
        },
        "deviceId": "ws-events-device"
    })
}

#[tokio::test]
async fn an_object_written_over_rpc_reaches_the_websocket_subscriber() {
    let (_dir, ark, server) = production_ws_fixture().await;
    let shutdown = server.shutdown_handle();
    let port = server.port();
    let task = tokio::spawn(server.run());
    let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;

    let mut params = note_write("ws-event-note");
    params["id"] = serde_json::json!("write-1");
    params["operation"] = serde_json::json!("objects.upsert");
    socket
        .send(Message::Text(params.to_string().into()))
        .await
        .unwrap();

    let mut wrote = false;
    let mut saw_change = None;
    for _ in 0..16 {
        let message = socket.next().await.unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        if value.get("id") == Some(&serde_json::json!("write-1")) {
            assert_eq!(value["ok"], true, "the write itself must succeed: {value}");
            wrote = true;
        }
        if value["event"] == "objects.changed" && value["id"] == "ws-event-note" {
            saw_change = Some(value);
        }
        if wrote && saw_change.is_some() {
            break;
        }
    }
    assert!(wrote, "no response to objects.upsert");
    let change = saw_change.expect("objects.changed never reached the subscriber");
    assert_eq!(change["typeId"], "core.note");
    assert_eq!(change["op"], "upsert");

    drop(socket);
    shutdown.shutdown().await.unwrap();
    task.await.unwrap().unwrap();
    shutdown_test_ark(ark).await;
}
