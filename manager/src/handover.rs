//! Who owns the global hotkey: the Manager while its window lives, the Engine
//! once the Manager is gone.
//!
//! Two listeners fire one key twice, and two toggles are no toggle at all, so
//! exactly one side hears the key at a time.

use serde_json::json;

/// The Engine drops its helper: the Manager keeps the tap, because the person
/// granted Input Monitoring to the program they see.
pub fn from_engine() {
    std::thread::spawn(|| {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let _ = engine.rpc("dictation.release_hotkey", json!({}));
    });
}

/// The Manager is closing: the Engine takes the hotkey back, so dictation keeps
/// working with only the menu bar icon left. Done before the window goes away,
/// and briefly — a quit waits for this answer.
pub fn to_engine(listening: bool) {
    if !listening {
        return;
    }
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let started = std::time::Instant::now();
        let result = engine.rpc("dictation.restore_hotkey", json!({}));
        let _ = done.send(format!(
            "hand over -> {} ({} ms)",
            match &result {
                Ok(_) => "ok".to_owned(),
                Err(error) => format!("failed: {error}"),
            },
            started.elapsed().as_millis(),
        ));
    });
    // A quiet Engine must not hold the window open.
    match wait.recv_timeout(std::time::Duration::from_millis(400)) {
        Ok(line) => crate::hotkey::log(&line),
        Err(_) => crate::hotkey::log("hand over -> no answer, the Engine keeps its old listener"),
    }
}
