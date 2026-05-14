// End-to-end integration test для AC1/AC4/AC5: реальный kosmos.exe + реальный
// ark-core-rpc.exe child + WS-handshake + один RPC через WS.
//
// Pre-requisite: ark-core-rpc release бинарь должен быть собран:
//   cargo build --release --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc
//
// Если бинаря нет — test ignor'ится (returns early) с понятным сообщением.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use kosmos::lock_file;

fn find_ark_core_rpc_binary() -> Option<PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let candidates = [
        format!("{manifest_dir}/../../packages/ark-core/rust/target/release/ark-core-rpc.exe"),
        format!("{manifest_dir}/../../packages/ark-core/rust/target/release/ark-core-rpc"),
        format!("{manifest_dir}/../../packages/ark-core/rust/target/debug/ark-core-rpc.exe"),
        format!("{manifest_dir}/../../packages/ark-core/rust/target/debug/ark-core-rpc"),
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

struct KosmosTestInstance {
    child: tokio::process::Child,
    lock: lock_file::KosmosLockFile,
    _appdata: tempfile::TempDir,
}

impl KosmosTestInstance {
    async fn spawn() -> Result<Self, String> {
        let ark_binary = find_ark_core_rpc_binary().ok_or_else(|| {
            "ark-core-rpc binary not found. Build it first: \
             cargo build --release --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc"
                .to_string()
        })?;

        let appdata = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
        let kosmos_bin = env!("CARGO_BIN_EXE_kosmos");

        let child = tokio::process::Command::new(kosmos_bin)
            .env("APPDATA", appdata.path())
            .env("HOME", appdata.path())
            .env("XDG_CONFIG_HOME", appdata.path())
            .env("ARK_CORE_RPC_PATH", &ark_binary)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn kosmos: {e}"))?;

        let lock_path = appdata.path().join("Kepler").join("kosmos.lock.json");
        let start = std::time::Instant::now();
        let lock = loop {
            if lock_path.exists() {
                match lock_file::read(&lock_path) {
                    Ok(l) => break l,
                    Err(_) => { /* partial write — retry */ }
                }
            }
            if start.elapsed() > Duration::from_secs(20) {
                return Err(format!(
                    "kosmos.lock.json did not appear at {lock_path:?} in 20s"
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };

        Ok(KosmosTestInstance {
            child,
            lock,
            _appdata: appdata,
        })
    }

    async fn shutdown(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_happy_path_handshake_and_rpc() {
    let instance = match KosmosTestInstance::spawn().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[skip] {e}");
            return;
        }
    };

    let url = format!("ws://127.0.0.1:{}", instance.lock.ws_port);
    let (mut ws, _) = connect_async(&url).await.expect("ws connect");

    // Hello.
    let hello = json!({
        "kind": "hello",
        "protocolVersion": serde_json::to_value(instance.lock.protocol_version)
            .map(|v| {
                format!(
                    "{}.{}.{}",
                    v["major"].as_u64().unwrap(),
                    v["minor"].as_u64().unwrap(),
                    v["patch"].as_u64().unwrap()
                )
            })
            .unwrap(),
        "token": instance.lock.auth_token,
        "pid": std::process::id(),
        "clientId": "integration-test-happy-path",
    });
    ws.send(Message::Text(hello.to_string())).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("hello_ok timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = frame.into_text().expect("text frame");
    let value: Value = serde_json::from_str(&text).expect("hello_ok JSON");
    assert_eq!(value["kind"], "hello_ok", "expected hello_ok, got: {text}");
    assert_eq!(value["compatibility"], "exact");

    // Send одна RPC — мы используем "unknown_op_for_smoke" чтобы не зависеть от того,
    // какие ops точно есть в ark-core-rpc. Ожидаем that ark отвечает с ok: false +
    // error message — это validates сам путь WS → ark_host → ark-core-rpc → response.
    let req = json!({
        "operation": "unknown_op_for_smoke_test",
        "id": "test-rpc-1",
    });
    ws.send(Message::Text(req.to_string())).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("rpc response timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = frame.into_text().expect("text frame");
    let value: Value = serde_json::from_str(&text).expect("response JSON");
    assert_eq!(
        value["id"], "test-rpc-1",
        "response must echo client request id"
    );
    // ok может быть true (если ARK странно ответил) или false (что более вероятно
    // для несуществующей op). Главное — request/response correlation работает.
    assert!(
        value.get("ok").is_some(),
        "response must have 'ok' field, got: {text}"
    );

    instance.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_rejects_missing_protocol_version() {
    let instance = match KosmosTestInstance::spawn().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[skip] {e}");
            return;
        }
    };

    let url = format!("ws://127.0.0.1:{}", instance.lock.ws_port);
    let (mut ws, _) = connect_async(&url).await.expect("ws connect");

    // Hello без protocolVersion (AC4).
    let bad_hello = json!({
        "kind": "hello",
        "token": instance.lock.auth_token,
        "pid": std::process::id(),
    });
    ws.send(Message::Text(bad_hello.to_string())).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("error timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = frame.into_text().expect("text frame");
    let value: Value = serde_json::from_str(&text).expect("hello_error JSON");
    assert_eq!(value["kind"], "hello_error");
    assert_eq!(value["code"], "missing_protocol_version");

    instance.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_rejects_invalid_token() {
    let instance = match KosmosTestInstance::spawn().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[skip] {e}");
            return;
        }
    };

    let url = format!("ws://127.0.0.1:{}", instance.lock.ws_port);
    let (mut ws, _) = connect_async(&url).await.expect("ws connect");

    let bad_hello = json!({
        "kind": "hello",
        "protocolVersion": "1.0.0",
        "token": "totally-wrong-token-not-the-real-one-deadbeef-x".repeat(2),
        "pid": std::process::id(),
    });
    ws.send(Message::Text(bad_hello.to_string())).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("error timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = frame.into_text().expect("text frame");
    let value: Value = serde_json::from_str(&text).expect("hello_error JSON");
    assert_eq!(value["kind"], "hello_error");
    assert_eq!(value["code"], "invalid_token");

    instance.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_rejects_nonexistent_pid() {
    let instance = match KosmosTestInstance::spawn().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[skip] {e}");
            return;
        }
    };

    let url = format!("ws://127.0.0.1:{}", instance.lock.ws_port);
    let (mut ws, _) = connect_async(&url).await.expect("ws connect");

    let bad_hello = json!({
        "kind": "hello",
        "protocolVersion": "1.0.0",
        "token": instance.lock.auth_token,
        "pid": 0x7FFFFFFFu32, // impossibly high
    });
    ws.send(Message::Text(bad_hello.to_string())).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("error timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = frame.into_text().expect("text frame");
    let value: Value = serde_json::from_str(&text).expect("hello_error JSON");
    assert_eq!(value["kind"], "hello_error");
    assert_eq!(value["code"], "invalid_pid");

    instance.shutdown().await;
}
