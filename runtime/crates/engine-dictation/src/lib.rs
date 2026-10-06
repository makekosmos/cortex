#![cfg_attr(test, allow(clippy::unwrap_used))]
// Windows is the primary target and its build sees every item; code that only
// Windows callers reach is dead elsewhere, so the lint is silenced off-Windows
// and in test builds, which carry platform-gated fixtures.
#![cfg_attr(any(test, feature = "test-support", not(windows)), allow(dead_code))]
// Same crate-level debt allows as `runtime/src/lib.rs` — this code moved
// verbatim from engine; the allowlist moves with it (KOS-334).
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
// did when this code lived inside the engine crate (KOS-334). Per-platform
// cfg decides which names are actually used.
#[allow(unused_imports)]
pub(crate) use engine_base::{brand, data_dir, file_hash, process_tree};

// Dictation (Phase 1, Groq cloud). См. spec в
// `.agent/tasks/2026-05-24-dictation/spec.md`.
//
// Wire format для WS broadcast events (flat JSON, см. forbidden.md):
//
// {"event":"dictation_state_changed",
//  "state":"idle|recording|transcribing|error","error":null|"..."}
//   {"event":"dictation_transcript","text":"...","language":"ru","durationMs":1234}
//   {"event":"dictation_config_changed"}
//
// Operations через WS под namespace `dictation.*` — см. `handle_dictation_op`
// в `host.rs` и intercept в `ws_server.rs`.

pub mod audio_duck;
pub mod config;
pub mod groq;
pub mod host;
#[cfg(windows)]
pub mod hotkey_hook;
pub mod inject;
pub mod local;
pub mod local_models;
pub mod local_sidecar;
pub mod local_sidecar_protocol;
pub mod local_whisper_dll;
#[cfg(target_os = "macos")]
pub mod macos_native;
pub(crate) mod model_sweep;
pub mod native_capture;
pub mod network;
pub mod pending;
pub mod retry;
pub mod stats;

pub use config::{
    config_path, data_dir, has_api_key, load, save, set_api_key, DictationConfig, InjectMode,
    NetworkProfile, TriggerMode,
};
pub use host::{handle_dictation_op, DictationHost, DictationResponse, DictationStateName};
