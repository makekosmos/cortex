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

pub mod agents;
pub mod app_index;
pub mod ark_host;
pub mod arrancador;
pub mod auth;
pub mod calculator;
pub mod command_bus;
pub mod crash_reporter;
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
pub mod observability;
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
pub mod protocol_usage;
pub mod protocol_version;
pub mod runtime_grants;
pub mod singleton;
pub mod store_catalog;
pub mod sync;
pub mod usage_tracker;
pub mod ws_server;
