#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, not(windows)), allow(dead_code))]
// Same crate-level debt allows as `runtime/src/lib.rs` — this code moved
// verbatim from engine; the allowlist moves with it (KOS-335).
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

// Shim: engine-base paths keep resolving as `crate::<module>` exactly as they
// did when this code lived inside the engine crate (KOS-335).
// `crate::brand` — engine_base::brand (privileged::brand is a separate
// service-branding submodule); `crate::priority` — BackgroundThreadGuard.
#[allow(unused_imports)]
pub(crate) use engine_base::{brand, priority};

pub mod app_index;
pub mod file_index;
pub mod privileged;

/// Engine install-root layout derived from an exe path — shared by
/// `privileged::firewall::validation` (was `crate::engine_versions::
/// engine_root_of_exe` before the split; engine re-exports it at the old
/// path).
pub mod install_layout;

/// Shared NTFS MFT-scan plumbing used by both `file_index::scanner::ntfs`
/// (user-mode fast path) and `privileged::ntfs_scan` (service-side scan).
#[cfg(windows)]
pub(crate) mod ntfs_common;

/// Same-user process image query — lives in `engine-base` (KOS-342) so
/// `engine_base::auth::process_image_path` can wrap it without a circular
/// edge; `privileged::impersonate` keeps using it at `crate::process_image`.
#[cfg(windows)]
pub use engine_base::process_image;
