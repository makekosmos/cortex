#![cfg_attr(test, allow(clippy::unwrap_used))]

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
    auth, crash_reporter, db_backup,
    lock_file::{self, KeplerLockFile, LOCK_FILE_FORMAT_VERSION},
    protocol_version::{ProtocolVersion, PROTOCOL_VERSION},
    singleton::SingletonGuard,
    sync,
    usage_tracker::{self, UsageTrackerDiagnosticsState, UsageTrackerOpts},
    ws_server::WsServer,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

struct SetupState {
    ark: Arc<ArkHost>,
    ws: WsServer,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    lock_path: PathBuf,
    _singleton: SingletonGuard,
    // tracing-appender WorkerGuard. Drop'нется когда SetupState упадёт —
    // тогда background writer flush'нет очередь и завершится. Без guard'а
    // последние логи теряются перед exit'ом процесса.
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Инициализирует tracing с rolling daily file appender в
/// `<lock_dir>/logs/kepler-backend.<DATE>`. Возвращает WorkerGuard который
/// нужно держать живым (drop = flush + shutdown).
///
/// Env override: `RUST_LOG` controls filter (default `info`).
/// JSON output для machine parsing — bug bundle / external aggregation.
fn init_tracing(lock_dir: &std::path::Path) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let log_dir = lock_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "kepler-backend");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Two layers: JSON в file (для bundle), pretty в stderr (для dev).
    let file_layer = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(false)
        .with_current_span(false);

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    guard
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
        ark,
        ws,
        usage_diagnostics,
        lock_path,
        _singleton,
        _log_guard,
    } = state;

    tokio::spawn(async move {
        if let Err(e) = ws.run().await {
            eprintln!("[kepler-backend] WS server exited: {e}");
        }
    });

    // Phase E2: usage-tracker как in-process модуль. Gated через ENV
    // `KEPLER_USAGE_TRACKER=0` если нужно отключить (например, в тестах);
    // по умолчанию enabled.
    let usage_tracker_enabled = std::env::var("KEPLER_USAGE_TRACKER").as_deref() != Ok("0");
    if usage_tracker_enabled {
        let ark_for_tracker = ark.clone();
        let icon_cache_dir = lock_path
            .parent()
            .map(|p| p.join("app-icons"))
            .unwrap_or_else(|| PathBuf::from("app-icons"));
        let opts = UsageTrackerOpts::from_env().with_icon_cache_dir(icon_cache_dir);
        usage_tracker::spawn(ark_for_tracker, opts, usage_diagnostics.clone());
        eprintln!("[kepler-backend] usage_tracker spawned (in-process)");
    } else {
        eprintln!("[kepler-backend] KEPLER_USAGE_TRACKER=0 — usage_tracker disabled");
    }
    // Periodic DB backup. На каждом старте проверяем — если прошло >= 24h
    // с последнего, делаем online backup в `<data_dir>/backups/`. fire-and-forget
    // (background task), failure НЕ блокирует startup (см. db_backup module).
    {
        let ark_for_backup = ark.clone();
        let backup_dir = lock_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        tokio::spawn(async move {
            db_backup::maybe_backup_on_startup(ark_for_backup, backup_dir).await;
        });
    }

    // ARK Host остаётся живым через clone (или базовый Arc) до конца main.
    let _keep_ark_alive = ark;

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
    // NB: до init_tracing нельзя зваать tracing::info!. Banner печатается
    // в stderr через eprintln; tracing включается ниже после crash_reporter.
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

    // Install panic hook ASAP — любой последующий panic пишется в
    // <data_dir>/crashes/panic-*.log. Требует RUST_BACKTRACE=1 для
    // backtrace; Kepler shell сетит этот env при spawn'е backend.
    crash_reporter::install(lock_dir.clone());

    // Phase 4 bug-detection: structured logging. tracing init ДО любых
    // других steps чтобы info!/warn!/error! из setup'а попали в файл.
    let log_guard = init_tracing(&lock_dir);
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        protocol = ?PROTOCOL_VERSION,
        "kepler-backend starting"
    );

    let singleton_path = lock_dir.join("kepler-singleton.lock.db");

    // Acquire через OS-level SQLite WAL lock — единственный надёжный singleton.
    // Удаляем stale kepler.lock.json (если был) тут же, чтобы shell не прочёл
    // устаревший ws_port в окне между acquire и write_atomic ниже.
    //
    // См. postmortems.md § 2026-05-23 — Kepler: singleton conflict из-за pid reuse.
    let (_singleton, stale_pid) =
        kepler_backend::singleton::acquire_clearing_stale_lock(&lock_path, &singleton_path)?;
    if let Some(pid) = stale_pid {
        tracing::info!(stale_pid = pid, "discarded stale kepler.lock.json");
    }
    tracing::info!(path = ?singleton_path, "singleton acquired");

    let ark_binary = ark_host::resolve_ark_core_rpc_path()?;
    tracing::info!(binary = ?ark_binary, "ark-core-rpc resolved");

    let db_path = std::env::var("KOSMOS_DB_PATH")
        .unwrap_or_else(|_| lock_dir.join("ark.db").to_string_lossy().into_owned());
    tracing::info!(db_path = %db_path, "ark db path");

    let ark = Arc::new(ArkHost::spawn(&ark_binary, &db_path).await?);
    tracing::info!("ark-core-rpc spawned and initialized");

    if std::env::var("KEPLER_SKIP_SYNC").as_deref() == Ok("1") {
        tracing::info!("KEPLER_SKIP_SYNC=1 — start_sync пропущен");
    } else {
        let space_id = sync::resolve_space_id();
        let device_id = match sync::resolve_device_id(&lock_dir) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(error = %e, "device_id resolve failed; fallback 'kepler-fallback'");
                "kepler-fallback".to_string()
            }
        };
        let device_name = sync::resolve_device_name();
        match sync::start_lan_sync(&ark, &space_id, &device_id, &device_name).await {
            Ok(()) => tracing::info!(
                space_id = %space_id,
                device_id = %device_id,
                device_name = ?device_name,
                "LAN sync started"
            ),
            Err(e) => tracing::warn!(
                error = %e,
                "start_sync failed; ARK ops продолжат работать, sync — нет"
            ),
        }
    }

    let token = auth::generate_token();
    let usage_diagnostics = Arc::new(UsageTrackerDiagnosticsState::default());

    // App Index: индексирует Start Menu + UWP. SQLite в lock_dir (рядом с ark.db),
    // icon cache в lock_dir/app-icons/. На старте — load cached синхронно (<10ms),
    // background rescan через spawn ниже.
    let app_index =
        match kepler_backend::app_index::AppIndex::new(&lock_dir, lock_dir.join("app-icons")) {
            Ok(ai) => std::sync::Arc::new(ai),
            Err(e) => {
                tracing::warn!(error = %e, "app_index init failed; launcher search будет пустой");
                // Создаём fallback с empty store — backend стартует, search возвращает [].
                // Если init упал жёстко, просто паникуем — это infrastructure failure.
                return Err(format!("app_index init failed: {e}").into());
            }
        };

    // Background rescan на старте — не блокирует bind / запуск backend'а.
    {
        let ai = app_index.clone();
        tokio::spawn(async move {
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

    // File Index v1: host-local filename/path search. Broad startup scans are
    // opt-in only; see postmortems.md § 2026-06-08.
    let file_index_enabled =
        kepler_backend::file_index::env_flag_enabled("KEPLER_FILE_INDEX", true);
    let file_index = match if file_index_enabled {
        kepler_backend::file_index::FileIndex::new(&lock_dir)
    } else {
        kepler_backend::file_index::FileIndex::new_disabled(&lock_dir)
    } {
        Ok(index) => std::sync::Arc::new(index),
        Err(e) => return Err(format!("file_index init failed: {e}").into()),
    };
    file_index.bind_self();

    let ws = WsServer::bind(
        ark.clone(),
        token.clone(),
        lock_dir.clone(),
        app_index.clone(),
        file_index.clone(),
        usage_diagnostics.clone(),
    )
    .await?;
    let port = ws.port();
    tracing::info!(port = port, "WS listening on 127.0.0.1");

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
    tracing::info!(path = ?lock_path, "lock-file written");

    // File indexing can be slow on large disks or when NTFS fast scan falls
    // back to walking. Start it only after WS + lock-file are ready, otherwise
    // shell IPC requests time out during backend startup.
    if file_index_enabled
        && kepler_backend::file_index::env_flag_enabled("KEPLER_FILE_INDEX_INITIAL_RESCAN", true)
        && file_index.has_roots()?
    {
        let index = file_index.clone();
        tokio::spawn(async move {
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
            initial_rescan = kepler_backend::file_index::env_flag_enabled(
                "KEPLER_FILE_INDEX_INITIAL_RESCAN",
                true
            ),
            has_roots = file_index.has_roots().unwrap_or(false),
            "file_index initial rescan skipped"
        );
    }

    Ok(SetupState {
        ark,
        ws,
        usage_diagnostics,
        lock_path,
        _singleton,
        _log_guard: log_guard,
    })
}
