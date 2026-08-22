use std::io::{BufRead, Write};

use serde_json::{json, Value};

fn main() {
    if let Some(path) = std::env::var_os("KOSMOS_FIXTURE_ENTRY_MARKER") {
        if let Ok(mut file) = std::fs::File::create(path) {
            let _ = file.write_all(b"entry:1\n");
            let _ = file.sync_all();
        }
    }
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let Some(Ok(line)) = lines.next() else {
        return;
    };
    drop(lines);
    let Ok(bootstrap) = serde_json::from_str::<Value>(&line) else {
        return;
    };
    if let Some(path) = std::env::var_os("KOSMOS_FIXTURE_BOOTSTRAP_MARKER") {
        use std::fs::OpenOptions;
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "bootstrap:1");
            let _ = file.sync_all();
        }
    }
    let package_id = bootstrap["package_id"].as_str().unwrap_or_default();
    if package_id.ends_with(".initial-fail") {
        return;
    }
    if package_id.ends_with(".malformed") {
        println!("sensitive-token-path=C:\\secret\\worker-token");
        eprintln!("sensitive-token-path=C:\\secret\\worker-token");
        return;
    }
    let token = bootstrap["token"].as_str().unwrap_or_default();
    let token = if package_id.ends_with(".wrong-token") {
        "wrong-token"
    } else {
        token
    };
    let package_id_out = if package_id.ends_with(".wrong-id") {
        "wrong.id"
    } else {
        package_id
    };
    let hello = json!({
        "method": "worker.hello",
        "package_id": package_id_out,
        "version": bootstrap["version"],
        "hash": bootstrap["hash"],
        "pid": bootstrap["pid"],
        "api_version": bootstrap["api_version"],
        "token": token,
    });
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{hello}").ok();
    out.flush().ok();
    if package_id.ends_with(".secret-fail") {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let failure = json!({
            "method": "worker.invalid",
            "token": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "body": "ARK_MARKDOWN_BODY_UNIQUE",
            "content": "MARKDOWN_CONTENT_UNIQUE",
            "payload": "RAW_REQUEST_PAYLOAD_UNIQUE",
            "secret": "WORKER_SECRET_UNIQUE",
            "vault_path": "C:\\Users\\secret-user\\vault\\private-note.md",
            "package_path": "C:\\Users\\secret-user\\packages\\fixture.kspkg",
        })
        .to_string();
        eprintln!("{failure}");
        std::thread::sleep(std::time::Duration::from_millis(100));
        writeln!(out, "{failure}").ok();
        out.flush().ok();
        return;
    }
    if package_id.ends_with(".wrong-token") || package_id.ends_with(".wrong-id") {
        return;
    }
    if package_id.ends_with(".crash") {
        std::thread::sleep(std::time::Duration::from_millis(200));
        return;
    }
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                match value.get("method").and_then(Value::as_str) {
                    Some("worker.stop") => {
                        let _ = stop_tx.send(());
                        break;
                    }
                    Some("worker.result") => {
                        let _ = result_tx.send(value);
                    }
                    _ => {}
                }
            }
        }
    });
    if package_id.ends_with(".ark-write") {
        let call = json!({
            "method": "worker.call",
            "id": "ark-write-1",
            "generation": bootstrap["generation"],
            "token": bootstrap["token"],
            "operation": "ark.write",
            "params": {
                "operation": "upsert_object_type",
                "params": {
                    "object_type": {
                        "id": "worker_fixture_type",
                        "name": "Worker Fixture",
                        "schemaJson": "{}",
                        "uiSchemaJson": "{}",
                        "createdAt": "2026-01-01T00:00:00Z",
                        "updatedAt": "2026-01-01T00:00:00Z",
                        "systemLocked": false
                    },
                    "device_id": "worker-fixture"
                }
            }
        });
        writeln!(out, "{call}").ok();
        out.flush().ok();
        let _ = result_rx.recv_timeout(std::time::Duration::from_secs(5));
    }
    loop {
        if stop_rx.try_recv().is_ok() {
            return;
        }
        let heartbeat = json!({
            "method": "worker.heartbeat",
            "generation": bootstrap["generation"],
            "token": bootstrap["token"],
        });
        writeln!(out, "{heartbeat}").ok();
        out.flush().ok();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
