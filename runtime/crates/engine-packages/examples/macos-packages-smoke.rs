//! Development smoke test for locally built, trusted integration archives.
//! Uses the real PackageStore extraction and checks native worker hello/stop.
//! Production launch ownership is tested separately by the supervisor suite.
use engine_base::package_manifest::VersionedManifest;
use engine_packages::package_store::PackageStore;
use serde_json::{json, Value};
use std::{error::Error, path::PathBuf, process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err("this smoke command requires macOS Apple Silicon".into());
    }
    let index_path = PathBuf::from(std::env::args().nth(1).ok_or(
        "usage: cargo run -p engine-packages --example macos-packages-smoke -- <packages.json>",
    )?);
    let index: Value = serde_json::from_slice(&std::fs::read(&index_path)?)?;
    let rows = index["packages"].as_array().ok_or("missing packages")?;
    if rows.is_empty() {
        return Err("no packages to check".into());
    }
    let temp = tempfile::tempdir()?;
    let store = PackageStore::new(temp.path())?;
    let parent = index_path.parent().ok_or("index has no parent")?;
    for row in rows {
        if row["os"] != "macos" || row["arch"] != "arm64" {
            return Err("index contains a foreign platform archive".into());
        }
        let manifest: VersionedManifest = serde_json::from_value(row["manifest"].clone())?;
        let artifact = row["artifact"].as_str().ok_or("missing artifact")?;
        let expected = format!("{}-{}-macos-arm64.kspkg", manifest.id(), manifest.version());
        if artifact != expected {
            return Err("unexpected artifact name".into());
        }
        let archive = parent.join(artifact);
        let installed = store.install_versioned(
            &archive,
            row["size"].as_u64().ok_or("missing size")?,
            row["sha256"].as_str().ok_or("missing hash")?,
            &manifest,
            2,
        )?;
        let executable = store.immutable_entrypoint(&installed)?;
        let work = tempfile::tempdir()?;
        let vault = work.path().join("vault");
        let state = work.path().join("state");
        std::fs::create_dir_all(&vault)?;
        std::fs::create_dir_all(&state)?;
        // Only development binaries built from this repository are run here;
        // no provider credentials, worker.run, network or ARK writes are used.
        let mut child = tokio::process::Command::new(executable)
            .env_clear()
            .current_dir(work.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let pid = child.id().ok_or("worker has no PID")?;
        let mut input = child.stdin.take().ok_or("missing stdin")?;
        let mut output = BufReader::new(child.stdout.take().ok_or("missing stdout")?).lines();
        let mut handles = serde_json::Map::new();
        let mut values = serde_json::Map::new();
        let settings = row["manifest"]["integration"]["settings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for setting in &settings {
            let key = setting["key"].as_str().ok_or("setting without key")?;
            match setting["kind"].as_str() {
                Some("secret" | "api_key" | "token") => {
                    handles.insert(key.into(), json!("abcdef0123456789".repeat(4)));
                }
                _ => {
                    values.insert(key.into(), json!("smoke-fixture"));
                }
            }
        }
        let bootstrap = json!({
            "method":"worker.bootstrap", "package_id":manifest.id(),
            "version":manifest.version(), "hash":installed.hash, "pid":pid,
            "api_version":1, "generation":1, "correlation_id":"local-smoke",
            "token":"local-smoke-token",
            "integration": {
                "settings":settings, "values":values, "secret_handles":handles,
                "account_key":"0123456789abcdef".repeat(4),
                "data_origin":"sportdata-dre.things.dbankcloud.com", "site_id":7
            },
            "bridge_config": {
                "vault_root":vault, "state_root":state,
                "selected_types":["com.mundus.note"],
                "editable_fields":["title", "body"], "readonly_fields":[]
            }
        });
        input.write_all(format!("{bootstrap}\n").as_bytes()).await?;
        input.flush().await?;
        let line = tokio::time::timeout(Duration::from_secs(5), output.next_line())
            .await??
            .ok_or("worker exited before hello")?;
        let hello: Value = serde_json::from_str(&line)?;
        if hello["method"] != "worker.hello"
            || hello["package_id"] != manifest.id()
            || hello["pid"] != pid
            || hello["hash"] != installed.hash
            || hello["token"] != "local-smoke-token"
        {
            return Err(format!("{} returned an invalid hello", manifest.id()).into());
        }
        input
            .write_all(b"{\"method\":\"worker.stop\",\"generation\":1,\"reason\":\"smoke\"}\n")
            .await?;
        input.flush().await?;
        drop(input);
        let status = tokio::time::timeout(Duration::from_secs(5), child.wait()).await??;
        if !status.success() {
            return Err(format!("{} failed to stop: {status}", manifest.id()).into());
        }
        println!(
            "{}: installed, native hello validated, clean stop",
            manifest.id()
        );
    }
    println!("Checked {} native macOS archives", rows.len());
    Ok(())
}
