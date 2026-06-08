#![cfg_attr(test, allow(clippy::unwrap_used))]

// Kosmos Kepler backend — headless ARK host runtime.
//
// Этот крейт extracted из apps/kepler/ в Phase 0 Kepler-Electron pivot. Содержит
// весь backend layer без UI: WS server, ark-core-rpc supervisor, singleton,
// lock-file discovery, протокол handshake, auth, LAN sync wiring.
//
// Используется двумя binaries:
//   * `kepler-backend` (этот крейт) — headless, для Electron Kepler child.
//   * `kepler` (apps/kepler/) — legacy desktop binary с tray + launcher.
//
// Оба binary запускают идентичный setup; UI binary добавляет tray + launcher
// поверх; headless binary только ждёт Ctrl+C / parent kill.

pub mod app_index;
pub mod ark_host;
pub mod arrancador;
pub mod auth;
pub mod command_bus;
pub mod crash_reporter;
pub mod db_backup;
pub mod diagnostics;
pub mod dictation;
pub mod export;
pub mod file_index;
pub mod focus;
pub mod lock_file;
pub mod pomodoro_host;
pub mod protocol_version;
pub mod singleton;
pub mod sync;
pub mod usage_tracker;
pub mod ws_server;
