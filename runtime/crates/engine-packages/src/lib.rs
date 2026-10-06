#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, feature = "test-support", not(windows)), allow(dead_code))]
// Same crate-level debt allows as `runtime/src/lib.rs` — this code moved
// verbatim from engine; the allowlist moves with it (KOS-336).
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

//! Package runtime extracted from `engine` (KOS-336): package store, worker
//! process/broker/supervisor/protocol, runtime grants and grant authority.
//! `engine` re-exports every module so `crate::<module>` paths keep working.

// Shim: engine-base paths keep resolving as `crate::<module>` exactly as they
// did when this code lived inside the engine crate (same trick as
// engine-dictation, KOS-334). Per-platform cfg decides which names are used.
#[cfg(windows)]
#[allow(unused_imports)]
pub(crate) use engine_base::win32;
#[allow(unused_imports)]
pub(crate) use engine_base::{
    auth, brand, data_dir, engine_dispatch, file_hash, handle_relative_fs, lock_file,
    observability, package_manifest, priority, process_tree, protocol_version,
};

// `crate::dictation::*` inside supervisor::engine_capability resolves to the
// engine-dictation crate.
pub(crate) use engine_dictation as dictation;

pub mod ark_host;
pub mod background_task;
pub mod catalog;
pub mod grant_authority;
pub mod native_apps;
pub mod package_launch;
pub mod package_registration;
pub mod package_service;
pub mod package_store;
pub mod package_worker_broker;
pub mod package_worker_process;
pub mod package_worker_protocol;
pub mod package_worker_secrets;
pub mod package_worker_supervisor;
pub mod runtime_grants;
