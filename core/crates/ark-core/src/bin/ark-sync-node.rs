use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ark_core::db::{init_schema, open_db, SqliteStorageBackend};
use ark_core::iroh_transport::{load_or_generate_secret_key, IrohConfig, IrohTransport};
use ark_core::relay_sync::{trim_process_heap, RelaySync, RelaySyncConfig};
use ark_core::sync_server::StorageBackend;
use ark_core::sync_transport::SyncTransport;
use serde_json::json;

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn optional_file_env(name: &str) -> Result<Option<String>, String> {
    let Ok(path) = env::var(name) else {
        return Ok(None);
    };
    let value = fs::read_to_string(&path).map_err(|e| format!("read {name} ({path}): {e}"))?;
    let value = value.trim().to_owned();
    Ok((!value.is_empty()).then_some(value))
}

fn atomic_write(path: &Path, value: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, value).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace {}: {e}", path.display()))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn serve() -> Result<(), String> {
    let db_path = PathBuf::from(required_env("MUNDUS_DB")?);
    let ticket_path = PathBuf::from(required_env("MUNDUS_TICKET_FILE")?);
    let device_id = required_env("MUNDUS_DEVICE_ID")?;
    let device_name = env::var("MUNDUS_DEVICE_NAME").unwrap_or_else(|_| device_id.clone());
    let space_id = required_env("MUNDUS_SPACE_ID")?;
    let auth_secret = optional_file_env("MUNDUS_AUTH_SECRET_FILE")?;
    let peer_ticket = optional_file_env("MUNDUS_PEER_TICKET_FILE")?;

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create database directory {}: {e}", parent.display()))?;
    }
    let conn = open_db(
        db_path
            .to_str()
            .ok_or_else(|| "MUNDUS_DB must be valid UTF-8".to_string())?,
    )?;
    init_schema(&conn)?;
    let secret_key = load_or_generate_secret_key(&conn)?;
    let shared = Arc::new(Mutex::new(conn));
    let storage = Arc::new(SqliteStorageBackend::new(shared));
    storage.set_device_id(&device_id)?;
    trim_process_heap();
    let transport = Arc::new(IrohTransport::new(IrohConfig {
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        space_id: space_id.clone(),
        secret_key: Some(secret_key),
        peer_addr: None,
        peer_ticket,
        relay_mode: None,
        auth_secret: auth_secret.clone(),
    }));
    let sync = RelaySync::with_transport(
        storage as Arc<dyn StorageBackend>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: space_id.clone(),
            device_id: device_id.clone(),
            device_name,
            auth_secret,
        },
        transport.clone() as Arc<dyn SyncTransport>,
    );
    sync.start().await?;
    let ticket = transport.our_ticket().await?;
    atomic_write(&ticket_path, &format!("{ticket}\n"))?;
    println!(
        "{}",
        json!({
            "status": "ready",
            "device_id": device_id,
            "space_id": space_id,
            "database": db_path,
            "ticket_file": ticket_path
        })
    );

    shutdown_signal().await;
    sync.stop();
    Ok(())
}

fn check_db() -> Result<(), String> {
    let db_path = required_env("MUNDUS_DB")?;
    let conn = open_db(&db_path)?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let freelist_count: i64 = conn
        .query_row("PRAGMA freelist_count", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    println!(
        "{}",
        json!({
            "database": db_path,
            "quick_check": integrity,
            "page_count": page_count,
            "freelist_count": freelist_count
        })
    );
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = match env::args().nth(1).as_deref().unwrap_or("serve") {
        "serve" => serve().await,
        "check-db" => check_db(),
        other => Err(format!("unknown command {other:?}; use serve or check-db")),
    };
    if let Err(error) = result {
        eprintln!("ark-sync-node: {error}");
        std::process::exit(1);
    }
    // Iroh owns background tasks whose runtime teardown may outlive systemd's
    // stop window. All SQLite writes are committed before the signal reaches
    // here, so terminate the storage-only daemon deterministically.
    std::process::exit(0);
}
