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

/// Minimal HTTP/1.1 POST: the engine API's own client is not a dependency of
/// the tests, and one request needs one connection.
async fn post_json(port: u16, path: &str, token: &str, body: &str) -> (u16, serde_json::Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect to the engine API");
    let request = format!(
        "POST {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\n\
         authorization: Bearer {token}\r\nx-kosmos-client-pid: {}\r\n\
         x-kosmos-api-version: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        std::process::id(),
        API_VERSION,
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read response");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .expect("status line");
    let payload = text.split("\r\n\r\n").nth(1).expect("response body").trim();
    (status, serde_json::from_str(payload).expect("json body"))
}

#[tokio::test]
async fn an_object_written_over_http_reaches_another_websocket_subscriber() {
    let (_dir, ark, server) = production_ws_fixture().await;
    let token = "test-token".repeat(8);
    let dispatcher = server.dispatcher();
    let ws_shutdown = server.shutdown_handle();
    let ws_port = server.port();
    let ws_task = tokio::spawn(server.run());
    // The subscriber is a different connection from the writer.
    let mut socket = authenticated_socket(ws_port, &token).await;

    let (api_dir, api) = crate::engine_api::EngineApiServer::bind_with_test_dispatcher(
        token.clone(),
        Arc::new(dispatcher),
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("engine API binds");
    let api_port = api.port();
    let api_shutdown = api.shutdown_handle();
    let api_task = tokio::spawn(api.run());

    let body = serde_json::json!({
        "operation": "objects.upsert",
        "object": {
            "id": "http-event-note",
            "typeId": "core.note",
            "typeVersion": "0.1.0",
            "title": "written over http",
            "contentJson": {"type": "doc", "content": []},
            "propsJson": {"description": null, "extensions": {}},
            "createdAt": "2026-10-16T00:00:00.000Z",
            "updatedAt": "2026-10-16T00:00:00.000Z",
            "deletedAt": null
        },
        "deviceId": "http-events-device"
    })
    .to_string();
    let (status, response) = post_json(api_port, "/v1/rpc", &token, &body).await;
    assert_eq!(status, 200, "HTTP write failed: {response}");
    assert_eq!(response["ok"], true, "HTTP write failed: {response}");

    let mut seen = None;
    for _ in 0..16 {
        let message = socket.next().await.unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        if value["event"] == "objects.changed" && value["id"] == "http-event-note" {
            seen = Some(value);
            break;
        }
    }
    let change = seen.expect("the WS subscriber never saw objects.changed");
    assert_eq!(change["typeId"], "core.note");
    assert_eq!(change["op"], "upsert");

    drop(socket);
    api_shutdown.begin_shutdown().await;
    api_shutdown.shutdown().await.unwrap();
    api_task.await.unwrap().unwrap();
    drop(api_dir);
    ws_shutdown.shutdown().await.unwrap();
    ws_task.await.unwrap().unwrap();
    shutdown_test_ark(ark).await;
}
