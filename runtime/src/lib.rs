#![cfg_attr(test, allow(clippy::unwrap_used))]
#![allow(dead_code)]
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

pub mod agents;
pub mod app_index;
pub(crate) mod app_network;
pub mod ark_host;
pub mod auth;
pub mod brand;
pub mod build_info;
pub mod calculator;
pub mod command_bus;
pub mod crash_reporter;
pub mod data_dir;
pub mod db_backup;
pub mod desktop_authority;
pub mod diagnostics;
pub mod dictation;
pub mod engine_api;
pub mod engine_control;
pub mod engine_dispatch;
pub mod engine_settings;
pub mod engine_supervisor;
pub mod export;
pub mod file_index;
pub mod focus;
pub mod grant_authority;
pub mod handle_relative_fs;
pub mod integrations;
pub mod lock_file;
pub mod manager_api;
pub mod markdown_vault;
pub mod native_apps;
pub mod observability;
pub(crate) mod package_launch;
pub mod package_manifest;
pub mod package_registration;
pub mod package_service;
pub mod package_store;
pub mod package_trust;
pub mod package_worker_broker;
pub mod package_worker_process;
pub mod package_worker_protocol;
pub mod package_worker_secrets;
pub mod package_worker_supervisor;
pub mod pomodoro;
pub mod pomodoro_host;
pub mod priority;
pub mod privileged;
pub mod protocol_usage;
pub mod protocol_version;
pub mod runtime_grants;
pub mod singleton;
pub mod store_catalog;
pub mod sync;
#[cfg(test)]
mod test_links;
pub mod updater;
pub mod usage_tracker;
pub mod user_data;
pub mod ws_server;
