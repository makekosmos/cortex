#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, not(windows)), allow(dead_code))]
// Remaining debt: these signatures need context structs / boxed errors.
#![allow(clippy::too_many_arguments, clippy::result_large_err)]

// Mundus backend — headless ARK host runtime.
//
// Этот крейт extracted из Electron-шелла в Phase 0 pivot. Содержит
// весь backend layer без UI: WS server, in-process ARK host, singleton,
// lock-file discovery, протокол handshake, auth, LAN sync wiring.
//
// Используется двумя binaries:
//   * `mundus-engine` (этот крейт) — headless engine binary.
//   * `mundus` — legacy desktop binary с tray + launcher.
//
// Оба binary запускают идентичный setup; UI binary добавляет tray + launcher
// поверх; headless binary только ждёт Ctrl+C / parent kill.

// KOS-331: foundation modules (brand, data_dir, file hashing, process tree,
// lock file, handle-relative FS, package manifest, observability, …) live in
// the `engine-base` crate; re-exported wholesale so `crate::<module>` paths
// keep working unchanged.
pub use engine_base::*;

// Package runtime removed: the Engine no longer hosts legacy packages,
// worker processes, grants or the package store. The in-process ARK host and
// the native-app registry stay in the Engine.
pub mod ark_host;
pub mod native_apps;

pub mod appearance;
pub mod backend_tray;
pub mod command_bus;
pub mod crash_reporter;
pub mod db_backup;
pub mod desktop_authority;
pub(crate) mod device_name;
pub mod diagnostics;
pub use engine_dictation as dictation;
// KOS-335: file_index + app_index + privileged live in the
// `engine-indexes` crate (one crate keeps the file_index<->privileged NTFS
// cycle internal); re-exported so `crate::<module>` paths keep working.
pub use engine_indexes::{app_index, file_index, privileged};
pub mod engine_api;
pub mod engine_control;
pub mod engine_settings;
pub mod engine_supervisor;
pub mod engine_versions;
pub mod focus;
pub mod installer;
pub mod macos_dock;
pub mod manager_api;
pub mod markdown_vault;
pub mod pomodoro;
pub mod pomodoro_host;
pub mod protocol_usage;
pub mod storage_maintenance;
pub mod sync;
#[cfg(test)]
mod test_links;
pub mod updater;
pub mod usage_tracker;
pub mod user_data;
pub mod ws_server;
