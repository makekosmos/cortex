// Backend crash reporter.
//
// Регистрирует `std::panic::set_hook`, который пишет безопасные crash metadata
// и redacted backtrace в `<data_dir>/crashes/panic-<timestamp>.log`. Default
// hook не вызывается, потому что он печатает raw panic payload в stderr.
//
// Backtrace требует `RUST_BACKTRACE=1` env (выставляется Kepler shell при
// spawn'е backend, см. shell/electron/main.ts → spawnBackend).
//
// Файл-фрагмент состоит из:
//   * version (CARGO_PKG_VERSION)
//   * timestamp ISO 8601
//   * фиксированный marker вместо panic payload
//   * location (file:line:col)
//   * backtrace (если доступен)
//
// Sync I/O — panic возникает в неизвестном контексте, async runtime может
// быть undefined. fs::write — единственный safe primitive.

use std::backtrace::Backtrace;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initialize panic hook. Должен быть вызван early в main() ПЕРЕД любым
/// кодом который может panic'нуть. data_dir — base directory (parent от
/// `engine.lock.json`), внутри которого будет создан `crashes/`.
pub fn install(data_dir: PathBuf, correlation_id: String) {
    let crash_dir = data_dir.join("crashes");
    if let Err(e) = std::fs::create_dir_all(&crash_dir) {
        crate::observability::stderr(format!(
            "[crash-reporter] failed to create {crash_dir:?}: {e}"
        ));
        return;
    }
    if CRASH_DIR.set(crash_dir.clone()).is_err() {
        eprintln!("[crash-reporter] already installed, skipping");
        return;
    }

    drop(std::panic::take_hook());
    std::panic::set_hook(Box::new(move |info| {
        let log_path = generate_log_path();
        let body = format_panic_log(info, &correlation_id);
        if let Err(e) = write_log(&log_path, &body) {
            crate::observability::stderr(format!(
                "[crash-reporter] failed to write {log_path:?}: {e}"
            ));
        } else {
            crate::observability::stderr(format!("[crash-reporter] wrote crash log: {log_path:?}"));
        }
        // Default hook не вызываем: он повторно печатает raw panic payload в
        // stderr и обходит privacy boundary.
    }));
    crate::observability::stderr(format!(
        "[crash-reporter] installed, crashes go to {crash_dir:?}"
    ));
}

fn generate_log_path() -> PathBuf {
    let now = chrono::Utc::now();
    // Используем `.format` который безопасен на Windows (нет колонов).
    let stamp = now.format("%Y-%m-%dT%H-%M-%S%.3fZ");
    let filename = format!("panic-{stamp}.log");
    CRASH_DIR
        .get()
        .map(|d| d.join(&filename))
        .unwrap_or_else(|| PathBuf::from(filename))
}

fn write_log(path: &Path, body: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(body.as_bytes())?;
    file.flush()
}

/// Format panic info as multi-line text. Public для test покрытия —
/// panic hook сам трудно протестировать (test runner intercept'ит panic'и).
pub fn format_panic_log(info: &std::panic::PanicHookInfo<'_>, correlation_id: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "kepler-backend v{} crashed\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!("crash_id: {}\n", crate::observability::crash_id()));
    out.push_str(&format!("correlation_id: {correlation_id}\n"));
    out.push_str("component: engine-core\n");
    out.push_str(&format!("timestamp: {}\n", chrono::Utc::now().to_rfc3339()));
    out.push_str("message: [REDACTED_PANIC_PAYLOAD]\n");
    if let Some(loc) = info.location() {
        out.push_str(&format!(
            "location: {}:{}:{}\n",
            crate::observability::redact_text(loc.file()),
            loc.line(),
            loc.column()
        ));
    } else {
        out.push_str("location: <unknown>\n");
    }
    out.push_str("\n--- backtrace ---\n");
    out.push_str(&crate::observability::redact_text(&format!(
        "{}",
        Backtrace::capture()
    )));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::panic, reason = "test must trigger the panic hook")]
    fn format_panic_log_excludes_message_and_includes_location() {
        // Реалистичный способ получить PanicHookInfo для теста — catch_unwind
        // c custom hook'ом, который перехватит payload.
        use std::sync::Mutex;
        let captured: Mutex<Option<String>> = Mutex::new(None);
        // Не можем легко создать PanicHookInfo напрямую — это opaque struct
        // в std. Полагаемся на real panic + take_hook.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new({
            let cap_handle = &captured;
            // SAFETY: hook живёт только в этом тесте, captured переживёт.
            // Используем static-like Box::leak workaround — но проще
            // дёрнуть через AtomicPtr / Lazy. Минимальный вариант:
            let cap_ref: &'static Mutex<Option<String>> = unsafe {
                std::mem::transmute::<&Mutex<Option<String>>, &'static Mutex<Option<String>>>(
                    cap_handle,
                )
            };
            move |info| {
                let s = format_panic_log(info, "00000000-0000-4000-8000-000000000001");
                *cap_ref.lock().unwrap() = Some(s);
            }
        }));
        let result = std::panic::catch_unwind(|| panic!("test panic message xyz"));
        std::panic::set_hook(prev);
        assert!(result.is_err(), "panic should have happened");
        let log = captured
            .lock()
            .unwrap()
            .clone()
            .expect("hook should have captured");
        assert!(!log.contains("test panic message xyz"), "log: {log}");
        assert!(log.contains("[REDACTED_PANIC_PAYLOAD]"), "log: {log}");
        assert!(
            log.contains("correlation_id: 00000000-0000-4000-8000-000000000001"),
            "log: {log}"
        );
        assert!(log.contains("crash_id:"), "log: {log}");
        assert!(log.contains("kepler-backend v"), "log: {log}");
        assert!(log.contains("location:"), "log: {log}");
        assert!(log.contains("backtrace"), "log: {log}");
    }

    #[test]
    fn generate_log_path_uses_safe_chars() {
        // Без active CRASH_DIR файл будет в pwd с именем panic-<stamp>.log.
        let path = generate_log_path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("panic-"));
        assert!(name.ends_with(".log"));
        assert!(!name.contains(':'), "колоны запрещены на Windows: {name}");
    }
}
