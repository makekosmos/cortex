// Kepler Kosmos — bin entry point.
//
// Thread architecture (architectural change after first runtime test):
//
//   Main thread:   eframe launcher window event loop.
//                  Winit (под капотом у eframe) на Windows допускает side-thread
//                  только через `any_thread()` hack; на macOS/Linux строго main —
//                  поэтому самый совместимый вариант — отдать main thread launcher'у.
//                  Окно скрыто по умолчанию, показывается при Alt+Space.
//
//   Tokio runtime: запущен на side-threads (создаём `Runtime` явно вместо
//                  `#[tokio::main]`). Содержит ws_server + ctrl_c handler +
//                  start_sync.
//
//   Tray thread:   std::thread с tao event loop (`with_any_thread(true)` на Win)
//                  для tray icon + global hotkey (Alt+Space).
//
// Lifecycle:
//   1. setup() sync init через rt.block_on: paths, singleton, ark, start_sync,
//      WS bind, lock-file write.
//   2. Spawn ws_server task на tokio.
//   3. Spawn ctrl_c watcher — sets tray::SHUTDOWN_REQUESTED.
//   4. Spawn tray std::thread.
//   5. Main thread → launcher::run_launcher (blocks until shutdown signal).
//   6. Cleanup: lock-file remove + runtime shutdown.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use kosmos::{
    ark_host::{self, ArkHost},
    auth, launcher,
    lock_file::{self, KosmosLockFile, LOCK_FILE_FORMAT_VERSION},
    protocol_version::{ProtocolVersion, PROTOCOL_VERSION},
    singleton::SingletonGuard,
    sync, tray,
    ws_server::WsServer,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

struct SetupState {
    ark: Arc<ArkHost>,
    ws: WsServer,
    lock_path: PathBuf,
    port: u16,
    _singleton: SingletonGuard,
}

fn main() -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("kosmos-tokio")
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[kosmos] FATAL: tokio runtime build failed: {e}");
            return ExitCode::from(1);
        }
    };

    let state = match rt.block_on(setup()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[kosmos] FATAL setup: {e}");
            return ExitCode::from(1);
        }
    };

    let SetupState {
        ark,
        ws,
        lock_path,
        port,
        _singleton: singleton,
    } = state;

    // Spawn WS server task.
    rt.spawn(async move {
        if let Err(e) = ws.run().await {
            eprintln!("[kosmos] WS server exited: {e}");
        }
    });

    // Ctrl+C watcher — устанавливает shutdown flag (тот же flag используется
    // launcher'ом и tray'ом для координации graceful exit).
    rt.spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("[kosmos] Ctrl+C — shutdown signal");
            tray::SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        }
    });

    // Tray icon + global hotkey на отдельной std-thread (tao with_any_thread).
    let tooltip = format!(
        "Kepler Kosmos\npid {} • port {}",
        std::process::id(),
        port
    );
    let _tray = tray::spawn(tooltip);
    if _tray.is_some() {
        eprintln!("[kosmos] tray icon spawned");
    } else {
        eprintln!("[kosmos] tray icon unavailable (icon load failed или OS headless)");
    }

    eprintln!("[kosmos] ready. Ctrl+Shift+K — launcher; tray «Выход» — shutdown.");

    // MAIN THREAD: launcher event loop. Блокирует until shutdown signal.
    // Окно скрыто; показывается на Alt+Space (через tray::LAUNCHER_REQUESTS).
    launcher::run_launcher(ark.clone(), rt.handle().clone());

    // Launcher exited — propagate shutdown.
    tray::SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);

    eprintln!("[kosmos] launcher exited, cleaning up");

    if let Err(e) = std::fs::remove_file(&lock_path) {
        eprintln!("[kosmos] failed to remove lock-file: {e}");
    }
    eprintln!("[kosmos] bye");

    // Graceful shutdown tokio (2s timeout).
    rt.shutdown_timeout(Duration::from_secs(2));
    drop(singleton);

    ExitCode::SUCCESS
}

async fn setup() -> Result<SetupState, DynError> {
    eprintln!(
        "Kosmos v{} starting (protocol {})",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION
    );

    let lock_path = lock_file::default_lock_file_path()?;
    let lock_dir = lock_path
        .parent()
        .ok_or("lock-file path has no parent")?
        .to_path_buf();
    let singleton_path = lock_dir.join("kosmos-singleton.lock.db");

    if let Some(existing) = lock_file::read_if_alive(&lock_path)? {
        eprintln!(
            "[kosmos] another Kosmos is already running (pid {}, ws_port {})",
            existing.pid, existing.ws_port
        );
        return Err("singleton conflict via lock-file".into());
    }

    let _singleton = SingletonGuard::acquire(&singleton_path)?;
    eprintln!("[kosmos] singleton acquired: {singleton_path:?}");

    let ark_binary = ark_host::resolve_ark_core_rpc_path()?;
    eprintln!("[kosmos] ark-core-rpc binary: {ark_binary:?}");

    let db_path = std::env::var("KEPLER_DB_PATH")
        .unwrap_or_else(|_| lock_dir.join("ark.db").to_string_lossy().into_owned());
    eprintln!("[kosmos] db: {db_path}");

    let ark = Arc::new(ArkHost::spawn(&ark_binary, &db_path).await?);
    eprintln!("[kosmos] ark-core-rpc spawned and initialized");

    if std::env::var("KOSMOS_SKIP_SYNC").as_deref() == Ok("1") {
        eprintln!("[kosmos] KOSMOS_SKIP_SYNC=1 — пропускаем start_sync (sync отключён)");
    } else {
        let space_id = sync::resolve_space_id();
        let device_id = match sync::resolve_device_id(&lock_dir) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[kosmos] WARN device_id resolve failed: {e}; используем 'kosmos-fallback'");
                "kosmos-fallback".to_string()
            }
        };
        let device_name = sync::resolve_device_name();
        match sync::start_lan_sync(&ark, &space_id, &device_id, &device_name).await {
            Ok(()) => eprintln!(
                "[kosmos] LAN sync started: space_id={space_id} device_id={device_id} device_name={device_name:?}"
            ),
            Err(e) => eprintln!(
                "[kosmos] WARN start_sync failed: {e}; ARK ops продолжат работать, sync — нет"
            ),
        }
    }

    let token = auth::generate_token();

    let ws = WsServer::bind(ark.clone(), token.clone()).await?;
    let port = ws.port();
    eprintln!("[kosmos] WS listening on 127.0.0.1:{port}");

    let lock = KosmosLockFile {
        format_version: LOCK_FILE_FORMAT_VERSION,
        protocol_version: ProtocolVersion::CURRENT,
        pid: std::process::id(),
        ws_port: port,
        auth_token: token,
        started_at: Utc::now().to_rfc3339(),
        db_path: db_path.clone(),
    };
    lock_file::write_atomic(&lock_path, &lock)?;
    eprintln!("[kosmos] lock-file: {lock_path:?}");

    Ok(SetupState {
        ark,
        ws,
        lock_path,
        port,
        _singleton,
    })
}

// Sync helpers (resolve_space_id / resolve_device_id / resolve_device_name /
// start_lan_sync) переехали в `kosmos_backend::sync` в Phase 0 — оба binary
// (этот legacy desktop + новый headless services/kosmos-backend) их шарят.
