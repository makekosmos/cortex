#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(
    all(windows, feature = "windows-gui-subsystem"),
    windows_subsystem = "windows"
)]
#![allow(dead_code, clippy::useless_conversion)]

mod backend_tray;

// Mundus backend — native runtime and Windows tray owner.
//
// По умолчанию запускается как самостоятельный native supervisor. Внутренний
// `--core-worker` режим содержит сам runtime («Electron only renders, Rust does
// everything else»). Никакого UI: ни tray, ни launcher, ни окна. Только:
//   * singleton lock (одна копия mundus-engine на машину),
//   * open the ARK service in-process,
//   * WS server 127.0.0.1:<port>,
//   * lock-file `engine.lock.json` для discovery.
//   * start_sync в фоне (если не MUNDUS_SKIP_SYNC=1),
//
// Core не принадлежит Electron: закрытие desktop process его не завершает.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use engine::{
    ark_host::ArkHost,
    auth, crash_reporter, db_backup,
    dictation::DictationHost,
    engine_api::EngineApiServer,
    engine_control::{self, ControlMessage},
    engine_supervisor::{self, ProcessMode},
    lock_file::{self, EngineLockFile, ENGINE_LOCK_FILE_FORMAT_VERSION},
    manager_api::ManagerState,
    observability::{self, CORRELATION_ID_ENV},
    package_service::PackageService,
    package_worker_supervisor::{EngineCapabilityExecutor, PackageWorkerSupervisor},
    protocol_usage::ProtocolUsageStore,
    protocol_version::{API_VERSION, API_VERSION_CURRENT, PROTOCOL_VERSION},
    singleton::SingletonGuard,
    sync,
    usage_tracker::{self, UsageTrackerDiagnosticsState, UsageTrackerOpts},
    ws_server::WsServer,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

struct SetupState {
    ark: Arc<ArkHost>,
    package_workers: PackageWorkerSupervisor,
    ws: WsServer,
    api: EngineApiServer,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    usage_tracker_enabled: bool,
    engine_lock_path: PathBuf,
    _singleton: SingletonGuard,
    // tracing-appender WorkerGuard. Drop'нется когда SetupState упадёт —
    // тогда background writer flush'нет очередь и завершится. Без guard'а
    // последние логи теряются перед exit'ом процесса.
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
    auth_token: String,
    ws_port: u16,
    http_port: u16,
}

/// Инициализирует tracing с rolling daily file appender в
/// `<lock_dir>/logs/mundus-engine.<DATE>`. Возвращает WorkerGuard который
/// нужно держать живым (drop = flush + shutdown).
///
/// Env override: `RUST_LOG` controls filter (default `info`).
/// JSON output для machine parsing — bug bundle / external aggregation.
fn init_tracing(lock_dir: &std::path::Path) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let log_dir = lock_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "mundus-engine");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Two layers: JSON в file (для bundle), pretty в stderr (для dev).
    let file_layer = fmt::layer()
        .json()
        .with_writer(move || observability::RedactingWriter::new(non_blocking.clone()))
        .with_target(true)
        .with_thread_ids(false)
        .with_current_span(false);

    let stderr_layer = fmt::layer()
        .with_writer(|| observability::RedactingWriter::new(std::io::stderr()))
        .with_target(false)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    guard
}

/// Задержка (мс) перед запуском отложенного фонового maintenance-таска
/// (app_index / file_index initial rescan, db_backup). Уводит тяжёлую работу
/// со startup hot path, чтобы не насыщать CPU/IO в момент cold start. Override
/// через `env_key`; `0` — без задержки.
fn startup_delay_ms(env_key: &str, default_ms: u64) -> u64 {
    std::env::var(env_key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_ms)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `privileged <install|uninstall|status|run-service>` — dedicated
    // one-shot / SCM modes that never start the Engine runtime.
    if let Some(code) = engine::privileged::cli::run_if_privileged(&args) {
        return code;
    }
    run(args)
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn run(args: Vec<String>) -> ExitCode {
    match engine_supervisor::process_mode(args) {
        ProcessMode::Supervisor => return engine_supervisor::run_supervisor().await,
        ProcessMode::RestartCore => return engine_supervisor::restart_core(),
        ProcessMode::Shutdown => return engine_supervisor::shutdown(),
        ProcessMode::CoreWorker => {}
    }
    run_core_worker().await
}

async fn run_core_worker() -> ExitCode {
    let state = match setup().await {
        Ok(s) => s,
        Err(e) => {
            observability::stderr(format!("[mundus-engine] FATAL setup: {e}"));
            return ExitCode::from(1);
        }
    };

    let SetupState {
        ark,
        package_workers,
        ws,
        api,
        usage_diagnostics,
        usage_tracker_enabled,
        engine_lock_path,
        _singleton,
        _log_guard,
        auth_token,
        ws_port,
        http_port,
    } = state;
    let api_shutdown = api.shutdown_handle();
    let ws_shutdown = ws.shutdown_handle();
    let desktop_authority = ws.desktop_authority();
    let grant_authority = ws.grant_authority();

    let supervised = std::env::var("MUNDUS_ENGINE_SUPERVISED").as_deref() == Ok("1");
    let agents_shutdown = ws.agents_handle();
    let ws_task = tokio::spawn(async move {
        if let Err(e) = ws.run().await {
            observability::stderr(format!("[mundus-engine] WS server exited: {e}"));
        }
    });
    let api_task = tokio::spawn(async move {
        if let Err(e) = api.run().await {
            observability::stderr(format!("[mundus-engine] Engine HTTP server exited: {e}"));
        }
    });
    let control_result = run_core_worker_readiness(
        ADAPTER_READINESS_TIMEOUT,
        || probe_http_dispatch(http_port, &auth_token),
        || probe_api_v1_ws_dispatch(ws_port, &auth_token),
        supervised.then_some(|| async {
            engine_control::start_core_control()
                .await
                .map_err(|error| error.to_string())
        }),
    )
    .await;
    let mut control_commands = match control_result {
        Ok(commands) => commands,
        Err(error) => {
            observability::stderr(format!(
                "[mundus-engine] startup readiness/control failed: {error}"
            ));
            api_shutdown.begin_shutdown().await;
            ws_shutdown.begin_shutdown().await;
            let _ = api_task.await;
            let _ = ws_task.await;
            let _ = api_shutdown.shutdown().await;
            let _ = ws_shutdown.shutdown().await;
            return ExitCode::from(1);
        }
    };

    // Runtime resolves the persisted Engine setting before creating the task.
    // The environment seam is test-only and is never persisted.
    if usage_tracker_enabled {
        let ark_for_tracker = ark.clone();
        let icon_cache_dir = engine_lock_path
            .parent()
            .map(|p| p.join("app-icons"))
            .unwrap_or_else(|| PathBuf::from("app-icons"));
        let opts = UsageTrackerOpts::from_env().with_icon_cache_dir(icon_cache_dir);
        usage_tracker::spawn(ark_for_tracker, opts, usage_diagnostics.clone());
        usage_diagnostics.mark_running();
        eprintln!("[mundus-engine] usage_tracker spawned (in-process)");
    } else {
        eprintln!("[mundus-engine] usage_tracker disabled by Engine settings");
    }
    // Periodic DB backup. На каждом старте проверяем — если прошло >= 24h
    // с последнего, делаем online backup в `<data_dir>/backups/`. fire-and-forget
    // (background task), failure НЕ блокирует startup (см. db_backup module).
    {
        let ark_for_backup = ark.clone();
        let backup_dir = engine_lock_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let delay = startup_delay_ms("MUNDUS_BACKUP_DELAY_MS", 120_000);
        tokio::spawn(async move {
            // Уводим backup со startup hot path: тяжёлое копирование БД не должно
            // совпадать с cold-start CPU/IO burst. (Понижение приоритета самого
            // копирования внутри ark-core service — Phase 2, см. spec.)
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            db_backup::maybe_backup_on_startup(ark_for_backup, backup_dir).await;
        });
    }

    // ARK Host остаётся живым через clone (или базовый Arc) до конца main.
    let _keep_ark_alive = ark;

    let (backend_tray, mut tray_events) = backend_tray::start();
    eprintln!("[mundus-engine] ready. Ctrl+C для shutdown.");

    let mut tray_exit_requested = false;
    let requested_control = if let Some(mut receiver) = control_commands.take() {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break None,
                event = tray_events.recv() => match event {
                    Some(backend_tray::TrayEvent::Exit) => {
                        tray_exit_requested = true;
                        break Some(ControlMessage::ShutdownRequested);
                    }
                    None => break None,
                },
                command = receiver.recv() => match command {
                    Some(ControlMessage::DesktopLease { electron_pid, credential }) => {
                        desktop_authority.register(
                            receiver.session_id().to_owned(),
                            receiver.generation(),
                            electron_pid,
                            &credential,
                        );

                        if let Err(error) = engine_control::notify_from_env(
                            ControlMessage::DesktopLeaseInstalled {
                                generation: receiver.generation(),
                                electron_pid,
                            },
                        ).await {
                            eprintln!("[mundus-engine] desktop lease acknowledgement failed: {error}");
                        }
                    }
                    Some(ControlMessage::DesktopLeaseRevoked { generation }) => {
                        grant_authority.close_generation(receiver.session_id(), generation);
                        desktop_authority.revoke_generation(receiver.session_id(), generation);
                    }
                    other => break other,
                },
            }
        }
    } else {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => None,
            event = tray_events.recv() => match event {
                Some(backend_tray::TrayEvent::Exit) => {
                    tray_exit_requested = true;
                    Some(ControlMessage::ShutdownRequested)
                }
                None => None,
            },

        }
    };
    eprintln!("[mundus-engine] shutdown signal received, cleaning up");
    backend_tray.stop();

    // Stop new HTTP/WS work before draining runtime-owned processes.
    api_shutdown.begin_shutdown().await;
    ws_shutdown.begin_shutdown().await;
    let _ = api_task.await;
    let _ = ws_task.await;
    let http_shutdown_result = api_shutdown.shutdown().await;
    let ws_shutdown_result = ws_shutdown.shutdown().await;

    if let Some(agents) = agents_shutdown.get() {
        agents.shutdown().await;
    }
    if let Err(error) = package_workers.stop_all().await {
        tracing::error!(target: "package_worker", error, "package worker cleanup failed during shutdown");
    }

    if matches!(
        requested_control,
        Some(ControlMessage::RestartRequested | ControlMessage::ShutdownRequested)
    ) {
        let _ = engine_control::notify_from_env(ControlMessage::CoreStopping).await;
    }

    if let Err(e) = std::fs::remove_file(&engine_lock_path) {
        observability::stderr(format!(
            "[mundus-engine] failed to remove Engine lock-file: {e}"
        ));
    }
    // MIGRATION(KOS-267): remove after 2026-11-01.
    engine::data_dir::remove_legacy_lock_shim();
    drop(_singleton);
    eprintln!("[mundus-engine] bye");
    if let Err(error) = http_shutdown_result {
        observability::stderr(format!("[mundus-engine] HTTP shutdown failed: {error}"));
    }
    if let Err(error) = ws_shutdown_result {
        observability::stderr(format!("[mundus-engine] WS shutdown failed: {error}"));
    }
    // Tray Exit is explicit user intent: cleanup failures are logged above but
    // must still surface TRAY_EXIT_CODE so supervisor/Desktop quit.
    if tray_exit_requested {
        ExitCode::from(engine_supervisor::TRAY_EXIT_CODE)
    } else if http_shutdown_result.is_err() || ws_shutdown_result.is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn run_core_worker_readiness<HF, WF, CF, H, W, C, T>(
    timeout: std::time::Duration,
    http_probe: H,
    ws_probe: W,
    control_start: Option<C>,
) -> Result<Option<T>, String>
where
    HF: std::future::Future<Output = Result<(), String>>,
    WF: std::future::Future<Output = Result<(), String>>,
    CF: std::future::Future<Output = Result<T, String>>,
    H: Fn() -> HF,
    W: Fn() -> WF,
    C: FnOnce() -> CF,
    T: Send,
{
    startup_orchestration(timeout, http_probe, ws_probe, control_start).await
}

fn legacy_migration_recovery_pending(data_dir: &std::path::Path) -> bool {
    [
        "com.kosmos.arcadia",
        "com.kosmos.memoria",
        "com.kosmos.agenda",
    ]
    .into_iter()
    .any(|target| {
        let root = data_dir.join("legacy-migrations").join("v1").join(target);
        if root.join("before").join("package-state.pending").is_file() {
            return true;
        }
        let journal = root.join("journal.json");
        let Ok(bytes) = std::fs::read(journal) else {
            return false;
        };
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| {
                value
                    .get("phase")
                    .and_then(serde_json::Value::as_str)
                    .map(|phase| phase == "prepared")
            })
            .unwrap_or(false)
    })
}

async fn setup() -> Result<SetupState, DynError> {
    // NB: до init_tracing нельзя зваать tracing::info!. Banner печатается
    // в stderr через eprintln; tracing включается ниже после crash_reporter.
    eprintln!(
        "mundus-engine v{} starting (protocol {})",
        engine::build_info::display_version(),
        PROTOCOL_VERSION
    );

    let correlation_id = observability::correlation_id();
    std::env::set_var(CORRELATION_ID_ENV, &correlation_id);
    let engine_lock_path = lock_file::default_engine_lock_file_path()?;
    let lock_dir = engine_lock_path
        .parent()
        .ok_or("lock-file path has no parent")?
        .to_path_buf();

    // Install panic hook ASAP — любой последующий panic пишется в
    // <data_dir>/crashes/panic-*.log. Требует RUST_BACKTRACE=1 для
    // backtrace; Mundus shell сетит этот env при spawn'е backend.
    crash_reporter::install(lock_dir.clone(), correlation_id.clone());

    // Phase 4 bug-detection: structured logging. tracing init ДО любых
    // других steps чтобы info!/warn!/error! из setup'а попали в файл.
    let log_guard = init_tracing(&lock_dir);
    tracing::info!(
        version = engine::build_info::display_version(),
        protocol = ?PROTOCOL_VERSION,
        correlation_id = %correlation_id,
        "mundus-engine starting"
    );
    // MIGRATION(KOS-267): remove after 2026-11-01. Brand-migration runs lazily
    // inside mundus_data_dir() above — before tracing init — so the report is
    // replayed here into the log file.
    if let Some(report) = engine::data_dir::last_report() {
        if report.roaming_migrated || report.local_entries_moved > 0 || report.fell_back_to_legacy {
            tracing::info!(
                roaming_migrated = report.roaming_migrated,
                local_entries_moved = report.local_entries_moved,
                fell_back_to_legacy = report.fell_back_to_legacy,
                "brand data-dir migration report"
            );
        }
    }

    // MIGRATION(KOS-267): on the legacy dir the singleton name must stay the
    // legacy one so a still-running 0.9.x Engine excludes this process.
    let singleton_path = lock_dir.join(engine::data_dir::singleton_lock_name(&lock_dir));

    // Acquire through the OS-level SQLite WAL singleton gate.
    let _singleton = engine::singleton::SingletonGuard::acquire(&singleton_path)?;
    if engine_lock_path.exists() {
        std::fs::remove_file(&engine_lock_path)?;
        tracing::info!("discarded stale engine.lock.json");
    }
    tracing::info!(path = ?singleton_path, "singleton acquired");

    let db_path = std::env::var("MUNDUS_DB_PATH")
        .unwrap_or_else(|_| lock_dir.join("ark.db").to_string_lossy().into_owned());
    tracing::info!(db_path = %db_path, "ark db path");

    let ark = Arc::new(ArkHost::open(&db_path).await?);
    tracing::info!("ark service opened in-process");

    let token = auth::generate_token();
    let usage_diagnostics = Arc::new(UsageTrackerDiagnosticsState::default());
    let test_override = (std::env::var("MUNDUS_TEST_MODE").as_deref() == Ok("1"))
        .then(|| std::env::var("MUNDUS_USAGE_TRACKER").ok())
        .flatten()
        .and_then(|value| match value.as_str() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        });
    let usage_startup = engine::engine_settings::resolve_usage_tracker(&lock_dir, test_override);
    usage_diagnostics.configure(usage_startup.enabled);
    tracing::info!(
        enabled = usage_startup.enabled,
        source = ?usage_startup.source,
        "usage tracker startup policy resolved"
    );
    let protocol_usage = Arc::new(ProtocolUsageStore::open(&lock_dir)?);
    let mut package_service = PackageService::open(&lock_dir)?;
    let mut worker_roots = vec![lock_dir.clone()];
    worker_roots.extend(
        [
            "USERPROFILE",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "PROGRAMDATA",
        ]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_dir()),
    );
    if let Some(value) = std::env::var_os("MUNDUS_WORKER_FILESYSTEM_ROOTS") {
        worker_roots.extend(std::env::split_paths(&value).filter(|path| path.is_dir()));
    }
    worker_roots.sort();
    worker_roots.dedup();
    let dictation_host = DictationHost::new(lock_dir.clone());
    let manager_state = ManagerState::new(lock_dir.clone());
    let updater = manager_state.updater();
    let package_workers = PackageWorkerSupervisor::with_ark_executor(
        API_VERSION_CURRENT.major.into(),
        Arc::new(EngineCapabilityExecutor::new(
            ark.clone(),
            dictation_host.clone(),
            manager_state.clone(),
        )),
    );
    package_service.configure_workers(
        package_workers.clone(),
        worker_roots,
        correlation_id.clone(),
    );
    if legacy_migration_recovery_pending(&lock_dir) {
        tracing::warn!("deferring package worker restore until legacy migration recovery");
    } else {
        package_service.restore_enabled_workers().await?;
    }
    let package_service = Arc::new(package_service);

    // Hotkey hooks are Engine-owned; forward their normalized trigger to the
    // installed Dictation worker so Desktop is never part of the control path.
    let mut dictation_events = dictation_host.subscribe();
    let dictation_packages = package_service.clone();
    tokio::spawn(async move {
        loop {
            match dictation_events.recv().await {
                Ok(event) => {
                    if event.get("event").and_then(serde_json::Value::as_str)
                        != Some("dictation.trigger")
                    {
                        continue;
                    }
                    let params = serde_json::json!({
                        "kind": event.get("kind").and_then(serde_json::Value::as_str),
                        "phase": event.get("phase").and_then(serde_json::Value::as_str),
                    });
                    if let Err(error) = dictation_packages
                        .invoke_worker_operation("dictation.trigger", params)
                        .await
                    {
                        tracing::debug!(error = %error, "dictation trigger worker unavailable");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // App Index: индексирует Start Menu + UWP. SQLite в lock_dir (рядом с ark.db),
    // icon cache в lock_dir/app-icons/. На старте — load cached синхронно (<10ms),
    // background rescan через spawn ниже.
    let app_index = match engine::app_index::AppIndex::new(&lock_dir, lock_dir.join("app-icons")) {
        Ok(ai) => std::sync::Arc::new(ai),
        Err(e) => {
            tracing::warn!(error = %e, "app_index init failed; launcher search будет пустой");
            // Создаём fallback с empty store — backend стартует, search возвращает [].
            // Если init упал жёстко, просто паникуем — это infrastructure failure.
            return Err(format!("app_index init failed: {e}").into());
        }
    };

    // File Index v1: host-local filename/path search. Broad startup scans are
    // opt-in only; see postmortems.md § 2026-06-08.
    let file_index_enabled = engine::file_index::env_flag_enabled("MUNDUS_FILE_INDEX", true);
    let file_index = match if file_index_enabled {
        engine::file_index::FileIndex::new(&lock_dir)
    } else {
        engine::file_index::FileIndex::new_disabled(&lock_dir)
    } {
        Ok(index) => std::sync::Arc::new(index),
        Err(e) => return Err(format!("file_index init failed: {e}").into()),
    };
    file_index.bind_self();

    let ws = WsServer::bind_with_hosts(
        ark.clone(),
        token.clone(),
        lock_dir.clone(),
        app_index.clone(),
        file_index.clone(),
        usage_diagnostics.clone(),
        protocol_usage.clone(),
        package_service.clone(),
        correlation_id.clone(),
        dictation_host,
        manager_state,
    )
    .await?;
    tokio::spawn(updater.run_startup_check_after_grace(ws.desktop_authority()));
    let port = ws.port();
    tracing::info!(port = port, "WS listening on 127.0.0.1");
    let dispatcher = Arc::new(ws.dispatcher());
    let api = EngineApiServer::bind(
        token.clone(),
        port,
        protocol_usage,
        correlation_id.clone(),
        package_service,
        dispatcher,
    )
    .await?;
    let http_port = api.port();
    tracing::info!(port = http_port, "Engine HTTP listening on 127.0.0.1");

    let started_at = Utc::now().to_rfc3339();
    let engine_lock = EngineLockFile {
        format_version: ENGINE_LOCK_FILE_FORMAT_VERSION,
        api_version: API_VERSION_CURRENT,
        pid: std::process::id(),
        http_port,
        ws_port: port,
        auth_token: token,
        started_at,
        correlation_id,
        engine_version: engine::build_info::engine_version().to_string(),
        source_commit: engine::build_info::engine_source_commit().to_string(),
    };
    lock_file::write_engine_atomic(&engine_lock_path, &engine_lock)?;
    // MIGRATION(KOS-267): remove after 2026-11-01.
    // Pinned component builds still discover the Engine only through
    // `%APPDATA%\Kosmos\engine.lock.json`; mirror the lock until repin.
    engine::data_dir::write_legacy_lock_shim(&engine_lock);
    tracing::info!(path = ?engine_lock_path, "Engine lock-file written");

    // LAN sync must not gate local readiness. If its fixed discovery port is
    // busy or slow, Eden/launcher still need immediate local ARK access.
    {
        let ark_for_sync = ark.clone();
        let lock_dir_for_sync = lock_dir.clone();
        tokio::spawn(async move {
            if std::env::var("MUNDUS_SKIP_SYNC").as_deref() == Ok("1") {
                tracing::info!("MUNDUS_SKIP_SYNC=1 — start_sync пропущен");
                return;
            }

            let space_id = sync::resolve_space_id();
            let device_id = match sync::resolve_device_id(&lock_dir_for_sync) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "device_id resolve failed; fallback 'kepler-fallback'"
                    );
                    "kepler-fallback".to_string()
                }
            };
            let device_name = sync::resolve_device_name();
            match sync::start_lan_sync(&ark_for_sync, &space_id, &device_id, &device_name).await {
                Ok(()) => {
                    tracing::info!(
                        space_id = %space_id,
                        device_id = %device_id,
                        device_name = ?device_name,
                        "LAN sync started"
                    );
                    sync::print_iroh_pairing_code_if_enabled(&ark_for_sync).await;
                }
                Err(e) => tracing::warn!(
                    error = %e,
                    "start_sync failed; ARK ops продолжат работать, sync — нет"
                ),
            }
        });
    }

    // App discovery is useful but not part of backend readiness. Start it only
    // after WS + lock-file are ready, otherwise extension IPC requests can time
    // out while the shell is still waiting for the Ark bridge.
    {
        let ai = app_index.clone();
        let delay = startup_delay_ms("MUNDUS_APP_INDEX_INITIAL_DELAY_MS", 15_000);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            match ai.rescan().await {
                Ok(stats) => tracing::info!(
                    added = stats.added,
                    updated = stats.updated,
                    removed = stats.removed,
                    total = stats.total,
                    "app_index initial rescan"
                ),
                Err(e) => tracing::warn!(error = %e, "app_index initial rescan failed"),
            }
        });
    }

    // File indexing can be slow on large disks or when NTFS fast scan falls
    // back to walking. Start it only after WS + lock-file are ready, otherwise
    // shell IPC requests time out during backend startup.
    if file_index_enabled
        && engine::file_index::env_flag_enabled("MUNDUS_FILE_INDEX_INITIAL_RESCAN", true)
        && file_index.has_roots()?
    {
        let index = file_index.clone();
        let delay = startup_delay_ms("MUNDUS_FILE_INDEX_INITIAL_DELAY_MS", 20_000);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            match index.rescan().await {
                Ok(stats) => tracing::info!(
                    total = stats.total,
                    roots = stats.roots,
                    exclude_noisy_folders = stats.exclude_noisy_folders,
                    respect_gitignore = stats.respect_gitignore,
                    include_hidden = stats.include_hidden,
                    ntfs_accelerated = stats.ntfs_accelerated,
                    "file_index initial rescan"
                ),
                Err(e) => tracing::warn!(error = %e, "file_index initial rescan failed"),
            }
        });
    } else {
        tracing::info!(
            enabled = file_index_enabled,
            initial_rescan =
                engine::file_index::env_flag_enabled("MUNDUS_FILE_INDEX_INITIAL_RESCAN", true),
            has_roots = file_index.has_roots().unwrap_or(false),
            "file_index initial rescan skipped"
        );
    }

    Ok(SetupState {
        ark,
        package_workers,
        ws,
        api,
        usage_diagnostics,
        usage_tracker_enabled: usage_startup.enabled,
        engine_lock_path,
        _singleton,
        _log_guard: log_guard,
        auth_token: engine_lock.auth_token.clone(),
        ws_port: port,
        http_port,
    })
}

const ADAPTER_READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

async fn wait_for_adapter_readiness(
    http_port: u16,
    ws_port: u16,
    token: &str,
) -> Result<(), String> {
    wait_for_adapter_readiness_with_timeout(http_port, ws_port, token, ADAPTER_READINESS_TIMEOUT)
        .await
}

async fn wait_for_adapter_readiness_with_timeout(
    http_port: u16,
    ws_port: u16,
    token: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    adapter_readiness_gate(
        timeout,
        || probe_http_dispatch(http_port, token),
        || probe_api_v1_ws_dispatch(ws_port, token),
        || async {},
    )
    .await
    .map_err(|_| "HTTP and API v1 WS probes did not become ready within 5s".to_string())
}

async fn adapter_readiness_gate<HF, WF, CF, H, W, C>(
    timeout: std::time::Duration,
    http_probe: H,
    ws_probe: W,
    core_ready: C,
) -> Result<(), ()>
where
    HF: std::future::Future<Output = Result<(), String>>,
    WF: std::future::Future<Output = Result<(), String>>,
    CF: std::future::Future<Output = ()>,
    H: Fn() -> HF,
    W: Fn() -> WF,
    C: Fn() -> CF,
{
    tokio::time::timeout(timeout, async {
        loop {
            let (http, ws) = tokio::join!(http_probe(), ws_probe());
            if http.is_ok() && ws.is_ok() {
                core_ready().await;
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(|_| ())?
}

async fn startup_orchestration<HF, WF, CF, H, W, C, T>(
    timeout: std::time::Duration,
    http_probe: H,
    ws_probe: W,
    control_start: Option<C>,
) -> Result<Option<T>, String>
where
    HF: std::future::Future<Output = Result<(), String>>,
    WF: std::future::Future<Output = Result<(), String>>,
    CF: std::future::Future<Output = Result<T, String>>,
    H: Fn() -> HF,
    W: Fn() -> WF,
    C: FnOnce() -> CF,
    T: Send,
{
    adapter_readiness_gate(timeout, http_probe, ws_probe, || async {})
        .await
        .map_err(|_| "HTTP and API v1 WS probes did not become ready within timeout".to_string())?;
    match control_start {
        Some(start) => Ok(Some(start().await?)),
        None => Ok(None),
    }
}

async fn probe_http_dispatch(port: u16, token: &str) -> Result<(), String> {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|e| e.to_string())?;
    let body = r#"{"_req_id":"readiness-http","operation":"diagnostics.snapshot"}"#;
    let request = format!(
        "POST /v1/rpc HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nx-kosmos-client-pid: {}\r\nx-kosmos-api-version: {API_VERSION}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        std::process::id(),
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|e| e.to_string())?;
    let response = String::from_utf8_lossy(&response);
    if response.starts_with("HTTP/1.1 200") && response.contains("\"ok\":true") {
        Ok(())
    } else {
        Err(format!("HTTP readiness response: {response}"))
    }
}

async fn probe_api_v1_ws_dispatch(port: u16, token: &str) -> Result<(), String> {
    let url = format!("{}127.0.0.1:{port}", "ws:".to_owned() + "//");
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|e| e.to_string())?;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "kind": "hello",
                "apiVersion": API_VERSION,
                "token": token,
                "pid": std::process::id(),
                "clientId": "mundus-runtime-readiness",
                "clientClass": "mundus-runtime",
                "clientVersion": engine::build_info::display_version(),
            })
            .to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(hello))) = socket.next().await else {
        return Err("API v1 WS closed during readiness hello".into());
    };
    if !hello.contains("\"kind\":\"hello_ok\"") {
        return Err(format!("API v1 WS hello rejected: {hello}"));
    }
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"_req_id":"readiness-ws","operation":"diagnostics.snapshot"}"#.into(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    while let Some(frame) = socket.next().await {
        let Ok(tokio_tungstenite::tungstenite::Message::Text(payload)) = frame else {
            continue;
        };
        if payload.contains("\"id\":\"readiness-ws\"") && payload.contains("\"ok\":true") {
            return Ok(());
        }
    }
    Err("API v1 WS closed before readiness response".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn raw_ws_handshake_case(port: u16, hello: Option<&str>, abrupt: bool) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("raw WS connection");
        if abrupt {
            socket
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n")
                .await
                .expect("partial upgrade");
            return;
        }
        socket
            .write_all(
                b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            )
            .await
            .expect("raw WS upgrade");
        let mut response = Vec::new();
        let mut chunk = [0_u8; 512];
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !response.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = socket.read(&mut chunk).await.expect("upgrade response");
                assert!(count > 0, "WS upgrade closed before response");
                response.extend_from_slice(&chunk[..count]);
            }
        })
        .await
        .expect("WS upgrade deadline");
        if let Some(hello) = hello {
            let payload = hello.as_bytes();
            assert!(payload.len() < 126);
            let mask = [7_u8, 11, 13, 17];
            let mut frame = vec![0x81, 0x80 | payload.len() as u8];
            frame.extend_from_slice(&mask);
            frame.extend(
                payload
                    .iter()
                    .enumerate()
                    .map(|(index, byte)| byte ^ mask[index % mask.len()]),
            );
            socket.write_all(&frame).await.expect("raw WS hello");
        }
    }

    async fn wait_for_ws_baseline(
        dispatcher: &engine::engine_dispatch::EngineDispatcher,
        command_bus: &Arc<engine::command_bus::CommandBus>,
        owners: usize,
        registrations: usize,
    ) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if dispatcher.live_owner_count() == owners
                    && command_bus.registration_count_sync() == registrations
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("WS owner cleanup deadline");
    }

    #[tokio::test]
    async fn startup_orchestration_calls_control_only_after_both_serving_adapters() {
        async fn run_case(http_ready: bool, ws_ready: bool) -> usize {
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let result = startup_orchestration(
                std::time::Duration::from_millis(40),
                move || async move {
                    http_ready
                        .then_some(())
                        .ok_or_else(|| "HTTP unavailable".to_string())
                },
                move || async move {
                    ws_ready
                        .then_some(())
                        .ok_or_else(|| "WS unavailable".to_string())
                },
                {
                    let calls = calls.clone();
                    Some(move || {
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        async { Ok::<&'static str, String>("CoreReady") }
                    })
                },
            )
            .await;
            let calls_count = calls.load(std::sync::atomic::Ordering::SeqCst);
            if http_ready && ws_ready {
                assert_eq!(result.unwrap(), Some("CoreReady"));
                assert_eq!(calls_count, 1);
            } else {
                assert!(result.is_err());
                assert_eq!(calls_count, 0);
            }
            calls_count
        }

        assert_eq!(run_case(false, false).await, 0);
        assert_eq!(run_case(true, false).await, 0);
        assert_eq!(run_case(false, true).await, 0);
        assert_eq!(run_case(true, true).await, 1);
    }

    #[tokio::test]
    async fn bound_but_not_serving_adapters_are_not_ready() {
        let http = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ws = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let result = wait_for_adapter_readiness_with_timeout(
            http.local_addr().unwrap().port(),
            ws.local_addr().unwrap().port(),
            &"t".repeat(64),
            std::time::Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn core_ready_gate_requires_both_serving_probes_and_emits_once() {
        for (http_ready, ws_ready) in [(true, false), (false, true), (true, true)] {
            let http_ready = Arc::new(std::sync::atomic::AtomicBool::new(http_ready));
            let ws_ready = Arc::new(std::sync::atomic::AtomicBool::new(ws_ready));
            let core_ready_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let result = adapter_readiness_gate(
                std::time::Duration::from_millis(60),
                {
                    let http_ready = http_ready.clone();
                    move || {
                        let ready = http_ready.load(std::sync::atomic::Ordering::Acquire);
                        async move { ready.then_some(()).ok_or_else(|| "HTTP withheld".into()) }
                    }
                },
                {
                    let ws_ready = ws_ready.clone();
                    move || {
                        let ready = ws_ready.load(std::sync::atomic::Ordering::Acquire);
                        async move { ready.then_some(()).ok_or_else(|| "WS withheld".into()) }
                    }
                },
                {
                    let core_ready_count = core_ready_count.clone();
                    move || {
                        core_ready_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        async {}
                    }
                },
            )
            .await;
            if http_ready.load(std::sync::atomic::Ordering::Acquire)
                && ws_ready.load(std::sync::atomic::Ordering::Acquire)
            {
                assert!(result.is_ok());
                assert_eq!(
                    core_ready_count.load(std::sync::atomic::Ordering::SeqCst),
                    1
                );
            } else {
                assert!(result.is_err());
                assert_eq!(
                    core_ready_count.load(std::sync::atomic::Ordering::SeqCst),
                    0
                );
            }
        }
    }

    #[tokio::test]
    async fn run_core_worker_readiness_requires_both_production_adapters_and_emits_authenticated_core_ready_once(
    ) {
        let dir = tempfile::tempdir().unwrap();
        let ark = Arc::new(
            ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
                .await
                .unwrap(),
        );
        let usage_diagnostics = Arc::new(UsageTrackerDiagnosticsState::default());
        let protocol_usage = Arc::new(ProtocolUsageStore::open(dir.path()).unwrap());
        let package_service = Arc::new(PackageService::open(dir.path()).unwrap());
        let ws = WsServer::bind(
            ark,
            "t".repeat(64),
            dir.path().to_path_buf(),
            Arc::new(
                engine::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).unwrap(),
            ),
            Arc::new(engine::file_index::FileIndex::new_disabled(dir.path()).unwrap()),
            usage_diagnostics,
            protocol_usage.clone(),
            package_service.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .unwrap();
        let ws_port = ws.port();
        let api = EngineApiServer::bind(
            "t".repeat(64),
            ws_port,
            protocol_usage,
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            Arc::new(ws.dispatcher()),
        )
        .await
        .unwrap();
        let http_port = api.port();
        let ws_dispatcher = ws.dispatcher();
        let ws_command_bus = ws.command_bus_handle();
        let ws_shutdown = ws.shutdown_handle();
        let ws_task = tokio::spawn(ws.run());
        let http_task = tokio::spawn(api.run());

        let baseline_owners = ws_dispatcher.live_owner_count();
        let baseline_registrations = ws_command_bus.registration_count_sync();
        raw_ws_handshake_case(ws_port, None, false).await;
        wait_for_ws_baseline(
            &ws_dispatcher,
            &ws_command_bus,
            baseline_owners,
            baseline_registrations,
        )
        .await;
        raw_ws_handshake_case(ws_port, Some("{"), false).await;
        wait_for_ws_baseline(
            &ws_dispatcher,
            &ws_command_bus,
            baseline_owners,
            baseline_registrations,
        )
        .await;
        let mut invalid_hello_value = serde_json::Map::new();
        invalid_hello_value.insert("kind".into(), serde_json::json!("hello"));
        invalid_hello_value.insert("apiVersion".into(), serde_json::json!("1.0"));
        invalid_hello_value.insert("token".into(), serde_json::json!("invalid"));
        invalid_hello_value.insert("pid".into(), serde_json::json!(std::process::id()));
        let invalid_hello = serde_json::Value::Object(invalid_hello_value).to_string();
        raw_ws_handshake_case(ws_port, Some(&invalid_hello), false).await;
        wait_for_ws_baseline(
            &ws_dispatcher,
            &ws_command_bus,
            baseline_owners,
            baseline_registrations,
        )
        .await;
        raw_ws_handshake_case(ws_port, None, true).await;
        wait_for_ws_baseline(
            &ws_dispatcher,
            &ws_command_bus,
            baseline_owners,
            baseline_registrations,
        )
        .await;

        let unavailable_port = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        for (http_probe_port, ws_probe_port) in
            [(http_port, unavailable_port), (unavailable_port, ws_port)]
        {
            let token = "t".repeat(64);
            let http_token = token.clone();
            let ws_token = token;
            let result = run_core_worker_readiness(
                std::time::Duration::from_millis(80),
                move || {
                    let token = http_token.clone();
                    async move { probe_http_dispatch(http_probe_port, &token).await }
                },
                move || {
                    let token = ws_token.clone();
                    async move { probe_api_v1_ws_dispatch(ws_probe_port, &token).await }
                },
                Some(|| async { Ok::<(), String>(()) }),
            )
            .await;
            assert!(result.is_err(), "one adapter must withhold CoreReady");
        }

        let core_secret = b"core-secret-for-test".to_vec();
        let (mut control, endpoint) = engine_control::ControlServer::bind(
            "socket-session".into(),
            1,
            "owner-1".into(),
            b"controller-secret".to_vec(),
            core_secret.clone(),
        )
        .await
        .expect("control server");
        let state = engine_control::ControlState {
            endpoint,
            supervisor_session_id: "socket-session".into(),
            child_generation: 1,
            owner_identity: "owner-1".into(),
            secret: engine_control::encode_secret(&core_secret),
        };
        let http_token = "t".repeat(64);
        let ws_token = http_token.clone();
        let core_commands = run_core_worker_readiness(
            std::time::Duration::from_secs(1),
            move || {
                let token = http_token.clone();
                async move { probe_http_dispatch(http_port, &token).await }
            },
            move || {
                let token = ws_token.clone();
                async move { probe_api_v1_ws_dispatch(ws_port, &token).await }
            },
            Some(move || async move {
                engine_control::start_core_control_with_secret(state, core_secret)
                    .await
                    .map_err(|error| error.to_string())
            }),
        )
        .await
        .expect("both production adapters must become ready")
        .expect("supervised readiness must return core commands");
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), control.recv())
                .await
                .expect("CoreReady deadline"),
            Some(ControlMessage::CoreReady)
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), control.recv())
                .await
                .is_err(),
            "CoreReady must not repeat"
        );
        drop(core_commands);
        control.abort();
        ws_shutdown.begin_shutdown().await;
        ws_task.await.unwrap().unwrap();
        ws_shutdown.shutdown().await.unwrap();
        http_task.abort();
    }

    #[test]
    fn packaged_windows_subsystem_keeps_debug_console_hidden() {
        let source = include_str!("main.rs");
        let crate_attributes = source.lines().take(8).collect::<Vec<_>>().join("\n");
        assert!(crate_attributes.contains("all(windows, feature = \"windows-gui-subsystem\")"));
        assert!(crate_attributes.contains("windows_subsystem = \"windows\""));

        let package_build = include_str!("../../desktop/scripts/build-backend.mjs");
        // The arg is a combined feature list (`windows-gui-subsystem,iroh-spike`).
        assert!(package_build.contains("--features\", \"windows-gui-subsystem,"));
        assert!(package_build.contains("iroh-spike"));
    }
}
