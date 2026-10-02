#![allow(clippy::unwrap_used)]

use engine::{
    app_index::AppIndex, ark_host::ArkHost, file_index::FileIndex, package_service::PackageService,
    protocol_usage::ProtocolUsageStore, usage_tracker::UsageTrackerDiagnosticsState,
    ws_server::WsServer,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{net::SocketAddr, process::Command, sync::Arc};
use tempfile::TempDir;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const GLOBAL_TOKEN: &str = "global-token";
const SESSION: &str = "desktop-session";
const GENERATION: u64 = 7;

struct Fixture {
    address: SocketAddr,
    authority: Arc<engine::desktop_authority::DesktopAuthorityRegistry>,
    grants: Arc<engine::grant_authority::GrantAuthorityRegistry>,
    snapshots: Arc<engine::package_worker_broker::SnapshotRegistry>,
    server: Option<WsServer>,
    // Last field: TempDir deletes its directory on drop, which on Windows
    // fails silently if anything still holds a file inside (the aborted
    // server task's ArkHost keeps ark.db/protocol.db open until it is fully
    // reaped). Dropping last plus awaiting the JoinHandle keeps the cleanup
    // deterministic.
    _dir: TempDir,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let ark = Arc::new(
        ArkHost::open(data_dir.join("ark.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let app_index = Arc::new(AppIndex::new(&data_dir, data_dir.join("app-icons")).unwrap());
    let file_index = Arc::new(FileIndex::new_disabled(&data_dir).unwrap());
    let package_service = Arc::new(PackageService::open(&data_dir).unwrap());
    let server = WsServer::bind(
        ark,
        GLOBAL_TOKEN.into(),
        data_dir,
        app_index,
        file_index,
        Arc::new(UsageTrackerDiagnosticsState::default()),
        Arc::new(ProtocolUsageStore::open(&dir.path().join("protocol.db")).unwrap()),
        package_service,
        "socket-test".into(),
    )
    .await
    .unwrap();
    let address = server.local_addr().unwrap();
    let authority = server.desktop_authority();
    let grants = server.grants_handle();
    let snapshots = server.snapshots_handle();
    Fixture {
        address,
        authority,
        grants,
        snapshots,
        server: Some(server),
        _dir: dir,
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(address: SocketAddr, pid: u32) -> (Socket, Value) {
    let (mut socket, _) = connect_async(format!("ws://{address}")).await.unwrap();
    socket
        .send(Message::Text(
            json!({
                "kind":"hello", "apiVersion":"1.0.0", "token":GLOBAL_TOKEN,
                "pid":pid, "clientClass":"kosmos-desktop", "clientVersion":"test"
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let response = socket.next().await.unwrap().unwrap();
    (
        socket,
        serde_json::from_str(response.to_text().unwrap()).unwrap(),
    )
}

async fn rpc(socket: &mut Socket, id: &str, operation: &str, params: Value) -> Value {
    let mut request = serde_json::Map::new();
    request.insert("id".into(), Value::String(id.into()));
    request.insert("operation".into(), Value::String(operation.into()));
    if operation == "desktop.authority.bind" {
        request.insert("params".into(), params);
    } else if let Some(object) = params.as_object() {
        request.extend(object.clone());
    }
    socket
        .send(Message::Text(Value::Object(request).to_string()))
        .await
        .unwrap();
    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap()
}

#[tokio::test]
async fn real_socket_desktop_authority_requires_exact_pid_credential_and_single_bind() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    fixture.authority.register(
        SESSION.into(),
        GENERATION,
        std::process::id(),
        "private-credential",
    );
    let task = tokio::spawn(server.run());
    let (mut socket, hello) = connect(fixture.address, std::process::id()).await;
    assert_eq!(hello["kind"], "hello_ok");
    assert_eq!(
        rpc(
            &mut socket,
            "wrong",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION,"credential":"wrong"})
        )
        .await["ok"],
        false
    );
    assert_eq!(
        rpc(
            &mut socket,
            "bind",
            "desktop.authority.bind",
            json!({"credential":"private-credential"})
        )
        .await["ok"],
        true
    );
    assert_eq!(
        rpc(
            &mut socket,
            "replay",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION,"credential":"private-credential"})
        )
        .await["ok"],
        false
    );
    drop(socket);
    fixture.authority.revoke_generation(SESSION, GENERATION);
    fixture.authority.revoke_generation(SESSION, GENERATION);
    assert_eq!(fixture.authority.len(), 0);
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
async fn real_socket_global_token_and_spoofed_class_cannot_reserve_snapshot() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    let authority = fixture.authority.clone();
    let task = tokio::spawn(server.run());
    let (mut socket, _) = connect(fixture.address, std::process::id()).await;
    let denied = rpc(
        &mut socket,
        "reserve",
        "package.snapshot.reserve",
        json!(
            {"packageId":"x",
            "source":"bundled",
            "root":"/secret",
            "path":"/secret",
            "identity":{"dev":1,
            "ino":2}}),
    )
    .await;
    assert_eq!(denied["ok"], false);
    assert_eq!(authority.len(), 0);
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
async fn real_socket_generation_replacement_revokes_old_owner_and_is_idempotent() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    fixture.authority.register(
        SESSION.into(),
        GENERATION,
        std::process::id(),
        "private-credential",
    );
    let task = tokio::spawn(server.run());
    let (mut old, hello) = connect(fixture.address, std::process::id()).await;
    assert_eq!(hello["kind"], "hello_ok");
    assert_eq!(
        rpc(
            &mut old,
            "bind",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION,"credential":"private-credential"})
        )
        .await["ok"],
        true
    );
    fixture.authority.register(
        SESSION.into(),
        GENERATION + 1,
        std::process::id(),
        "new-credential",
    );
    assert_eq!(
        fixture.authority.bind(
            SESSION,
            GENERATION,
            std::process::id(),
            "private-credential",
            1
        ),
        Err(engine::desktop_authority::AuthorityError::MissingLease)
    );
    assert_eq!(
        fixture.authority.bind(
            SESSION,
            GENERATION + 1,
            std::process::id(),
            "new-credential",
            2
        ),
        Ok(())
    );
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}

async fn wait_for_empty(fixture: &Fixture) {
    // Generous hang guard only — the drain is the event (KOS-308).
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            if fixture.grants.len() == 0 && fixture.snapshots.len() == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

fn assert_no_path(value: &Value, path: &str) {
    assert!(!value.to_string().contains(path));
}

#[tokio::test]
async fn real_socket_wrong_pid_is_denied_at_bind_and_has_no_authority() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    fixture.authority.revoke_generation(SESSION, GENERATION);
    // A direct child, not a shell wrapper: nextest flags this test as LEAK
    // when a process the test spawned outlives it. `cmd /C ping` left ping.exe
    // orphaned, and kill() alone leaves the child handle unreaped — kill +
    // wait is deterministic teardown.
    #[cfg(windows)]
    let mut child = Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    #[cfg(not(windows))]
    let mut child = Command::new("sleep").arg("5").spawn().unwrap();
    fixture
        .authority
        .register(SESSION.into(), GENERATION, child.id(), "lease-credential");
    let task = tokio::spawn(server.run());
    let (mut socket, hello) = connect(fixture.address, std::process::id()).await;
    assert_eq!(hello["kind"], "hello_ok");
    let denied = rpc(
        &mut socket,
        "bind",
        "desktop.authority.bind",
        json!({"sessionId":SESSION,"generation":GENERATION,"credential":"lease-credential"}),
    )
    .await;
    assert_eq!(denied["ok"], false);
    assert_eq!(denied["error"], "desktop authority denied");
    drop(socket);
    fixture.authority.revoke_generation(SESSION, GENERATION);
    let _ = child.kill();
    let _ = child.wait();
    wait_for_empty(&fixture).await;
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
async fn real_socket_authorized_grant_and_snapshot_are_opaque_and_disconnect_cleans_all() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    fixture.authority.register(
        SESSION.into(),
        GENERATION,
        std::process::id(),
        "grant-credential",
    );
    let root = fixture._dir.path().join("native-dialog");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("asset.bin"), b"real native dialog bytes").unwrap();
    let task = tokio::spawn(server.run());
    let (mut socket, _) = connect(fixture.address, std::process::id()).await;
    assert_eq!(
        rpc(
            &mut socket,
            "bind",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION,"credential":"grant-credential"})
        )
        .await["ok"],
        true
    );
    let registered = rpc(
        &mut socket,
        "grant",
        "grant.authority.register",
        json!(
            {"extensionId":"ext",
            "root":root.to_str().unwrap(),
            "provenance":"native-dialog",
            "exactFile":false}),
    )
    .await;
    assert_eq!(registered["ok"], true);
    assert_no_path(&registered, root.to_str().unwrap());
    let grant_id = registered["data"]["grantId"].as_str().unwrap();
    let reserved = rpc(
        &mut socket,
        "snapshot",
        "grant.snapshot.reserve",
        json!({"grantId":grant_id,"extensionId":"ext","relativeAsset":"asset.bin"}),
    )
    .await;
    assert_eq!(reserved["ok"], true);
    assert_no_path(&reserved, root.to_str().unwrap());
    let handle = reserved["data"]["handle"].as_str().unwrap().to_owned();
    let chunked = rpc(
        &mut socket,
        "chunk",
        "grant.snapshot.chunk",
        json!({"handle":handle,"offset":0,"length":24}),
    )
    .await;
    assert_eq!(chunked["ok"], true);
    assert_eq!(
        base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            chunked["data"]["bytes"].as_str().unwrap()
        )
        .unwrap(),
        b"real native dialog bytes"
    );
    assert_eq!(
        rpc(
            &mut socket,
            "close",
            "grant.snapshot.close",
            json!({"handle":handle})
        )
        .await["ok"],
        true
    );
    assert_eq!(fixture.authority.len(), 1);
    assert_eq!(fixture.grants.len(), 1);
    assert_eq!(fixture.snapshots.len(), 0);
    drop(socket);
    wait_for_empty(&fixture).await;
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
async fn real_socket_disconnect_cleans_open_snapshot_and_stale_handle_is_denied() {
    let mut fixture = fixture().await;
    let server = fixture.server.take().unwrap();
    let shutdown = server.shutdown_handle();
    fixture.authority.register(
        SESSION.into(),
        GENERATION,
        std::process::id(),
        "open-credential",
    );
    let root = fixture._dir.path().join("native-dialog");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("asset.bin"), b"open snapshot").unwrap();
    let task = tokio::spawn(server.run());
    let (mut socket, _) = connect(fixture.address, std::process::id()).await;
    assert_eq!(
        rpc(
            &mut socket,
            "bind",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION,"credential":"open-credential"})
        )
        .await["ok"],
        true
    );
    let grant = rpc(
        &mut socket,
        "grant",
        "grant.authority.register",
        json!({"extensionId":"ext","root":root.to_str().unwrap(),"provenance":"native-dialog"}),
    )
    .await;
    let handle = rpc(
        &mut socket,
        "snapshot",
        "grant.snapshot.reserve",
        json!({"grantId":grant["data"]["grantId"],"extensionId":"ext","relativeAsset":"asset.bin"}),
    )
    .await["data"]["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(socket);
    wait_for_empty(&fixture).await;
    fixture.authority.register(
        SESSION.into(),
        GENERATION + 1,
        std::process::id(),
        "fresh-credential",
    );
    let (mut fresh, _) = connect(fixture.address, std::process::id()).await;
    assert_eq!(
        rpc(
            &mut fresh,
            "bind",
            "desktop.authority.bind",
            json!({"sessionId":SESSION,"generation":GENERATION + 1,"credential":"fresh-credential"})
        )
        .await["ok"],
        true
    );
    assert_eq!(
        rpc(
            &mut fresh,
            "stale",
            "grant.snapshot.chunk",
            json!({"handle":handle,"offset":0,"length":32})
        )
        .await["ok"],
        false
    );
    drop(fresh);
    wait_for_empty(&fixture).await;
    let _ = shutdown.shutdown().await;
    let _ = task.await;
}
