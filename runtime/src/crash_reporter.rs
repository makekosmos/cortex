// Backend crash reporter.
//
// Регистрирует `std::panic::set_hook` который пишет panic info + backtrace в
// `<data_dir>/crashes/panic-<timestamp>.log`. После записи передаёт control
// default hook'у — стандартный stderr вывод + process exit как обычно.
//
// Backtrace требует `RUST_BACKTRACE=1` env (выставляется Kepler shell при
// spawn'е backend, см. shell/electron/main.ts → spawnBackend).
//
// Файл-фрагмент состоит из:
//   * version (CARGO_PKG_VERSION)
//   * timestamp ISO 8601
//   * panic message (info.payload() cast to &str / &String)
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
/// `kepler.lock.json`), внутри которого будет создан `crashes/`.
pub fn install(data_dir: PathBuf) {
    let crash_dir = data_dir.join("crashes");
    if let Err(e) = std::fs::create_dir_all(&crash_dir) {
        eprintln!("[crash-reporter] failed to create {crash_dir:?}: {e}");
        return;
    }
    if CRASH_DIR.set(crash_dir.clone()).is_err() {
        eprintln!("[crash-reporter] already installed, skipping");
        return;
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let log_path = generate_log_path();
        let body = format_panic_log(info);
        if let Err(e) = write_log(&log_path, &body) {
            eprintln!("[crash-reporter] failed to write {log_path:?}: {e}");
        } else {
            eprintln!("[crash-reporter] wrote crash log: {log_path:?}");
        }
        // Передаём control default'у — стандартный stderr вывод сохраняется.
        default_hook(info);
    }));
    eprintln!("[crash-reporter] installed, crashes go to {crash_dir:?}");
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
pub fn format_panic_log(info: &std::panic::PanicHookInfo<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "kepler-backend v{} crashed\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!("timestamp: {}\n", chrono::Utc::now().to_rfc3339()));
    let payload = info.payload();
    let message = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<panic payload of unknown type>".to_string()
    };
    out.push_str(&format!("message: {message}\n"));
    if let Some(loc) = info.location() {
        out.push_str(&format!(
            "location: {}:{}:{}\n",
            loc.file(),
            loc.line(),
            loc.column()
        ));
    } else {
        out.push_str("location: <unknown>\n");
    }
    out.push_str("\n--- backtrace ---\n");
    out.push_str(&format!("{}", Backtrace::capture()));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_panic_log_includes_message_and_location() {
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
                let s = format_panic_log(info);
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
        assert!(log.contains("test panic message xyz"), "log: {log}");
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
