// Kosmos Kepler backend — headless binary.
//
// Запускается как child процесс Electron Kepler main process в новой
// архитектуре («Electron only renders, Rust does everything else»). Никакого
// UI: ни tray, ни launcher, ни окна. Только:
//   * singleton lock (одна копия kepler-backend на машину),
//   * spawn ark-core-rpc child,
//   * start_sync (если не KEPLER_SKIP_SYNC=1),
//   * WS server 127.0.0.1:<port>,
//   * lock-file `kepler.lock.json` для discovery.
//
// Завершается на Ctrl+C / parent SIGTERM / shutdown signal от родителя.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::Utc;

use kepler_backend::{
    ark_host::{self, ArkHost},
    auth,
    lock_file::{self, KeplerLockFile, LOCK_FILE_FORMAT_VERSION},
    protocol_version::{ProtocolVersion, PROTOCOL_VERSION},
    singleton::SingletonGuard,
    sync,
    ws_server::WsServer,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

struct SetupState {
    _ark: Arc<ArkHost>,
    ws: WsServer,
    lock_path: PathBuf,
    _singleton: SingletonGuard,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> ExitCode {
    let state = match setup().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[kepler-backend] FATAL setup: {e}");
            return ExitCode::from(1);
        }
    };

    let SetupState {
        _ark,
        ws,
        lock_path,
        _singleton,
    } = state;

    tokio::spawn(async move {
        if let Err(e) = ws.run().await {
            eprintln!("[kepler-backend] WS server exited: {e}");
        }
    });

    eprintln!("[kepler-backend] ready. Ctrl+C для shutdown.");

    let _ = tokio::signal::ctrl_c().await;
    eprintln!("[kepler-backend] shutdown signal received, cleaning up");

    if let Err(e) = std::fs::remove_file(&lock_path) {
        eprintln!("[kepler-backend] failed to remove lock-file: {e}");
    }
    drop(_singleton);
    eprintln!("[kepler-backend] bye");
    ExitCode::SUCCESS
}

async fn setup() -> Result<SetupState, DynError> {
    eprintln!(
        "kepler-backend v{} starting (protocol {})",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION
    );

    let lock_path = lock_file::default_lock_file_path()?;
    let lock_dir = lock_path
        .parent()
        .ok_or("lock-file path has no parent")?
        .to_path_buf();
    let singleton_path = lock_dir.join("kepler-singleton.lock.db");

    if let Some(existing) = lock_file::read_if_alive(&lock_path)? {
        eprintln!(
            "[kepler-backend] another instance running (pid {}, ws_port {})",
            existing.pid, existing.ws_port
        );
        return Err("singleton conflict via lock-file".into());
    }

    let _singleton = SingletonGuard::acquire(&singleton_path)?;
    eprintln!("[kepler-backend] singleton acquired: {singleton_path:?}");

    let ark_binary = ark_host::resolve_ark_core_rpc_path()?;
    eprintln!("[kepler-backend] ark-core-rpc binary: {ark_binary:?}");

    let db_path = std::env::var("KOSMOS_DB_PATH")
        .unwrap_or_else(|_| lock_dir.join("ark.db").to_string_lossy().into_owned());
    eprintln!("[kepler-backend] db: {db_path}");

    let ark = Arc::new(ArkHost::spawn(&ark_binary, &db_path).await?);
    eprintln!("[kepler-backend] ark-core-rpc spawned and initialized");

    if std::env::var("KEPLER_SKIP_SYNC").as_deref() == Ok("1") {
        eprintln!("[kepler-backend] KEPLER_SKIP_SYNC=1 — start_sync пропущен");
    } else {
        let space_id = sync::resolve_space_id();
        let device_id = match sync::resolve_device_id(&lock_dir) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[kepler-backend] WARN device_id resolve failed: {e}; fallback 'kepler-fallback'");
                "kepler-fallback".to_string()
            }
        };
        let device_name = sync::resolve_device_name();
        match sync::start_lan_sync(&ark, &space_id, &device_id, &device_name).await {
            Ok(()) => eprintln!(
                "[kepler-backend] LAN sync started: space_id={space_id} device_id={device_id} device_name={device_name:?}"
            ),
            Err(e) => eprintln!(
                "[kepler-backend] WARN start_sync failed: {e}; ARK ops продолжат работать, sync — нет"
            ),
        }
    }

    let token = auth::generate_token();

    let ws = WsServer::bind(ark.clone(), token.clone()).await?;
    let port = ws.port();
    eprintln!("[kepler-backend] WS listening on 127.0.0.1:{port}");

    let lock = KeplerLockFile {
        format_version: LOCK_FILE_FORMAT_VERSION,
        protocol_version: ProtocolVersion::CURRENT,
        pid: std::process::id(),
        ws_port: port,
        auth_token: token,
        started_at: Utc::now().to_rfc3339(),
        db_path: db_path.clone(),
    };
    lock_file::write_atomic(&lock_path, &lock)?;
    eprintln!("[kepler-backend] lock-file: {lock_path:?}");

    Ok(SetupState {
        _ark: ark,
        ws,
        lock_path,
        _singleton,
    })
}
