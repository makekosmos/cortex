#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, feature = "test-support", not(windows)), allow(dead_code))]
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

//! Foundation modules extracted from `engine` (KOS-331).
//!
//! These modules carry no heavy dependencies and sit below the Engine
//! application layer; `engine` re-exports them wholesale via
//! `pub use engine_base::*` so existing `crate::<module>` paths keep working.

pub mod auth;
pub mod brand;
pub mod build_info;
pub mod build_metadata;
pub mod data_dir;
pub mod file_hash;
pub mod handle_relative_fs;
pub mod lock_file;
pub mod observability;
pub mod package_manifest;
pub mod priority;
/// Same-user process image query — lives here (not engine-indexes) so
/// `auth::process_image_path` can wrap it without a circular dep;
/// engine-indexes re-exports it for `privileged::impersonate`.
#[cfg(windows)]
pub mod process_image;
pub mod process_tree;
#[cfg(test)]
mod process_tree_tests;
pub mod protocol_version;
pub mod singleton;
// KOS-338: dispatch hub API (request/handler/dispatcher) extracted from
// engine so both the `engine-packages` registrants and the engine
// transports (engine_api/ws_server) share one boundary crate.
pub mod engine_dispatch;
#[cfg(windows)]
pub mod win32;
