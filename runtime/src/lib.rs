#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, not(windows)), allow(dead_code))]
#![allow(
    clippy::collapsible_if,
    clippy::derivable_impls,
    clippy::explicit_auto_deref,
    clippy::len_without_is_empty,
    clippy::manual_is_multiple_of,
    clippy::map_entry,
    clippy::needless_borrow,
    clippy::needless_match,
    clippy::needless_question_mark,
    clippy::needless_return,
    clippy::new_ret_no_self,
    clippy::new_without_default,
    clippy::question_mark,
    clippy::redundant_closure,
    clippy::redundant_locals,
    clippy::result_large_err,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_lazy_evaluations
)]

// `package-worker-fixture` relaxes the broker's HTTPS-only origin check to
// allow loopback http origins and exposes worker test hooks — none of that
// may ever ship. debug_assertions are off in every release profile, so this
// compile_error is the hard gate between the test feature and a shipped
// binary (KOS-270).
#[cfg(all(feature = "package-worker-fixture", not(debug_assertions)))]
compile_error!(
    "feature package-worker-fixture is test-only and cannot be enabled in a release build"
);

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

// KOS-336: package runtime (store, worker process/broker/supervisor/protocol,
// runtime grants, grant authority, worker secrets) plus the in-process ARK
// host live in the `engine-packages` crate; re-exported per-module so
// `crate::<module>` paths keep working unchanged.
// KOS-338: the dispatch hub (`engine_dispatch`) and `auth` moved to
// `engine-base` (re-exported by the glob above); package_service plus its
// support modules (catalog, package_launch, package_registration,
// background_task, native_apps) moved to `engine-packages`.
pub use engine_packages::{
    ark_host, background_task, catalog, grant_authority, native_apps, package_launch,
    package_registration, package_service, package_store, package_worker_broker,
    package_worker_process, package_worker_protocol, package_worker_secrets,
    package_worker_supervisor, runtime_grants,
};

pub mod agents;
pub(crate) mod app_network;
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
pub mod integrations;
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
