use super::*;
pub(super) async fn production_ws_fixture() -> (tempfile::TempDir, WsServer) {
    let dir = tempfile::tempdir().unwrap();

    let ark = Arc::new(
        crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
            .await
            .unwrap(),
    );
    let ws = WsServer::bind(
        ark,
        "test-token".repeat(8),
        dir.path().to_path_buf(),
        Arc::new(crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).unwrap()),
        Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).unwrap()),
        Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
        Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).unwrap()),
        Arc::new(crate::package_service::PackageService::open(dir.path()).unwrap()),
        "00000000-0000-4000-8000-000000000001".into(),
    )
    .await
    .unwrap();
    (dir, ws)
}

pub(super) async fn authenticated_socket(
    port: u16,
    token: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({
                "kind": "hello",
                "apiVersion": API_VERSION,
                "token": token,
                "pid": std::process::id(),
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    assert!(socket
        .next()
        .await
        .unwrap()
        .unwrap()
        .to_text()
        .unwrap()
        .contains("hello_ok"));
    socket
}
