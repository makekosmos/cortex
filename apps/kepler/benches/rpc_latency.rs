// AC6 latency benchmark — real Kepler + WS + ark-core-rpc round-trip.
//
// Цели (план, фаза 1):
//   P50 ≤ 5ms
//   P95 ≤ 15ms (soft target)
//   P95 > 30ms — HARD STOP, переключаемся на named-pipes / UDS.
//
// Cargo.toml имеет `[[bench]] name = "rpc_latency" harness = false`, поэтому здесь
// — обычный main(), не Criterion API. Это даёт нам полный контроль над процедурой
// бенча и точное reporting percentiles.
//
// Pre-requisite: ark-core-rpc release бинарь должен быть собран:
//   cargo build --release --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use kepler::lock_file;

const WARMUP_ITERS: usize = 50;
const MEASURE_ITERS: usize = 1000;

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

fn percentile(sorted: &[Duration], pct: f64) -> Duration {
    let idx = ((sorted.len() as f64) * pct).floor() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let ark_binary = match find_ark_core_rpc_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "[bench] ark-core-rpc binary not found. Build first:\n  \
                 cargo build --release --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc"
            );
            std::process::exit(2);
        }
    };

    let kepler_bin = env!("CARGO_BIN_EXE_kepler");
    let appdata = tempfile::tempdir().expect("tempdir");

    eprintln!("[bench] spawning kepler with ark-core-rpc at {ark_binary:?}");
    let mut child = tokio::process::Command::new(kepler_bin)
        .env("APPDATA", appdata.path())
        .env("HOME", appdata.path())
        .env("XDG_CONFIG_HOME", appdata.path())
        .env("ARK_CORE_RPC_PATH", &ark_binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn kepler");

    let lock_path = appdata.path().join("Kosmos").join("kepler.lock.json");
    let start = Instant::now();
    let lock = loop {
        if lock_path.exists() {
            if let Ok(l) = lock_file::read(&lock_path) {
                break l;
            }
        }
        if start.elapsed() > Duration::from_secs(20) {
            child.kill().await.ok();
            eprintln!("[bench] lock-file did not appear in 20s");
            std::process::exit(3);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    eprintln!(
        "[bench] kepler ready (pid {} on port {})",
        lock.pid, lock.ws_port
    );

    let url = format!("ws://127.0.0.1:{}", lock.ws_port);
    let (mut ws, _) = connect_async(&url).await.expect("ws connect");

    let pv = format!(
        "{}.{}.{}",
        lock.protocol_version.major, lock.protocol_version.minor, lock.protocol_version.patch
    );
    let hello = json!({
        "kind": "hello",
        "protocolVersion": pv,
        "token": lock.auth_token,
        "pid": std::process::id(),
        "clientId": "bench",
    });
    ws.send(Message::Text(hello.to_string())).await.unwrap();
    let _ack = ws.next().await.expect("hello_ok").unwrap();

    eprintln!(
        "[bench] handshake done. warmup {WARMUP_ITERS}, measure {MEASURE_ITERS}",
    );

    // Warmup
    for i in 0..WARMUP_ITERS {
        let req = json!({
            "operation": "unknown_op_for_bench",
            "id": format!("warm-{i}"),
        });
        ws.send(Message::Text(req.to_string())).await.unwrap();
        let _ = ws.next().await.unwrap().unwrap();
    }

    // Measure
    let mut samples: Vec<Duration> = Vec::with_capacity(MEASURE_ITERS);
    for i in 0..MEASURE_ITERS {
        let req = json!({
            "operation": "unknown_op_for_bench",
            "id": format!("m-{i}"),
        });
        let t = Instant::now();
        ws.send(Message::Text(req.to_string())).await.unwrap();
        let _ = ws.next().await.unwrap().unwrap();
        samples.push(t.elapsed());
    }

    samples.sort();
    let p50 = percentile(&samples, 0.50);
    let p90 = percentile(&samples, 0.90);
    let p95 = percentile(&samples, 0.95);
    let p99 = percentile(&samples, 0.99);
    let avg: Duration = samples.iter().sum::<Duration>() / (samples.len() as u32);
    let min = samples.first().copied().unwrap();
    let max = samples.last().copied().unwrap();

    println!("=== AC6 RPC latency benchmark ===");
    println!("samples : {}", samples.len());
    println!("min     : {min:?}");
    println!("avg     : {avg:?}");
    println!("P50     : {p50:?}");
    println!("P90     : {p90:?}");
    println!("P95     : {p95:?}");
    println!("P99     : {p99:?}");
    println!("max     : {max:?}");
    println!();

    // Gates
    let target_p50 = Duration::from_millis(5);
    let target_p95 = Duration::from_millis(15);
    let hard_p95 = Duration::from_millis(30);

    let mut exit_code = 0;

    if p95 > hard_p95 {
        eprintln!(
            "❌ AC6 HARD FAIL: P95 ({p95:?}) > 30ms — рассмотри переключение транспорта \
             на named-pipes (Win) / UDS (Mac/Linux). См. план, Decision #2."
        );
        exit_code = 1;
    } else if p95 > target_p95 {
        println!(
            "⚠ P95 ({p95:?}) > target {target_p95:?} (но < hard limit {hard_p95:?}) — \
             borderline, фиксируем в evidence, переходим дальше."
        );
    } else {
        println!("✅ P95 ({p95:?}) within target {target_p95:?}");
    }

    if p50 > target_p50 {
        println!("⚠ P50 ({p50:?}) > target {target_p50:?}");
    } else {
        println!("✅ P50 ({p50:?}) within target {target_p50:?}");
    }

    let _ = child.kill().await;
    std::process::exit(exit_code);
}
