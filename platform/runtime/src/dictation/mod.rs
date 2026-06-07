// Dictation (Phase 1, Groq cloud). См. spec в
// `.agent/tasks/2026-05-24-dictation/spec.md`.
//
// Wire format для WS broadcast events (flat JSON, см. forbidden.md):
//   {"event":"dictation_state_changed","state":"idle|recording|transcribing|error","error":null|"..."}
//   {"event":"dictation_transcript","text":"...","language":"ru","durationMs":1234}
//   {"event":"dictation_config_changed"}
//
// Operations через WS под namespace `dictation.*` — см. `handle_dictation_op`
// в `host.rs` и intercept в `ws_server.rs`.

pub mod config;
pub mod groq;
pub mod host;
#[cfg(windows)]
pub mod hotkey_hook;
pub mod inject;
#[cfg(target_os = "macos")]
pub mod macos_native;
pub mod network;
pub mod pending;
pub mod retry;
pub mod stats;

pub use config::{
    config_path, data_dir, has_api_key, load, save, set_api_key, DictationConfig, InjectMode,
    NetworkProfile, TriggerMode,
};
pub use host::{handle_dictation_op, DictationHost, DictationResponse, DictationStateName};
