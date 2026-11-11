// macOS native dictation helpers.
//
// Rust owns lifecycle/state and Swift owns low-level macOS APIs. Helpers are
// line-oriented JSON executables copied from sample/SuperCmd-main and built by
// `desktop/scripts/build-macos-native.mjs` (a no-op off macOS).

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use tokio::sync::broadcast;

use super::config::TriggerMode;

static WATCH_GENERATION: AtomicU64 = AtomicU64::new(0);
static CAPTURE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The Manager watches the keyboard itself now: it is the program the person
/// grants Input Monitoring to. Two listeners fire one key twice, and two
/// toggles cancel out, so the Engine drops its helper on request — and does not
/// start another one when the setting changes later.
pub fn release_listeners() {
    MANAGER_OWNS_HOTKEY.store(true, Ordering::SeqCst);
    WATCH_GENERATION.fetch_add(1, Ordering::SeqCst);
    CAPTURE_GENERATION.fetch_add(1, Ordering::SeqCst);
}

static MANAGER_OWNS_HOTKEY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
pub enum NativeHelperError {
    #[error("helper '{name}' not found in candidates: {candidates:?}")]
    NotFound {
        name: String,
        candidates: Vec<PathBuf>,
    },
}

pub fn set_hotkey_active(
    hotkey: &str,
    mode: TriggerMode,
    tx: broadcast::Sender<Value>,
) -> Result<(), NativeHelperError> {
    if MANAGER_OWNS_HOTKEY.load(Ordering::SeqCst) {
        WATCH_GENERATION.fetch_add(1, Ordering::SeqCst);
        return Ok(());
    }
    let spec = match parse_hotkey(hotkey) {
        Some(spec) => spec,
        None => {
            WATCH_GENERATION.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
    };
    let helper = resolve_helper("hotkey-hold-monitor")?;
    prompt_microphone();
    let generation = WATCH_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    thread::spawn(move || {
        while WATCH_GENERATION.load(Ordering::SeqCst) == generation {
            if let Err(error) = watch_hotkey_once(&helper, &spec, mode, &tx, generation) {
                let _ = tx.send(json!({
                    "event": "dictation_hotkey_error",
                    "platform": "macos",
                    "error": error,
                }));
                thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    });

    Ok(())
}

/// The mic dialog belongs to this app. A new signature has no grant, and a
/// capture that times out looks the same as a hotkey that did nothing.
fn prompt_microphone() {
    let Ok(helper) = resolve_helper("microphone-access") else {
        return;
    };
    let _ = Command::new(helper)
        .arg("--prompt")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn watch_hotkey_once(
    helper: &Path,
    spec: &MacHotkeySpec,
    mode: TriggerMode,
    tx: &broadcast::Sender<Value>,
    generation: u64,
) -> Result<(), String> {
    let args = vec![
        spec.key_code.to_string(),
        bool_arg(spec.cmd).to_string(),
        bool_arg(spec.ctrl).to_string(),
        bool_arg(spec.alt).to_string(),
        bool_arg(spec.shift).to_string(),
        bool_arg(spec.function).to_string(),
        bool_arg(spec.double_fn).to_string(),
    ];
    let mut child = Command::new(helper)
        .args(args)
        .env("MUNDUS_PARENT_PID", std::process::id().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let child_pid = child.id();
    let child_done = Arc::new(AtomicBool::new(false));
    {
        let child_done = child_done.clone();
        thread::spawn(move || {
            while !child_done.load(Ordering::SeqCst)
                && WATCH_GENERATION.load(Ordering::SeqCst) == generation
            {
                thread::sleep(std::time::Duration::from_millis(250));
            }
            if !child_done.load(Ordering::SeqCst) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(child_pid as libc::pid_t, libc::SIGTERM);
                }
            }
        });
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "missing helper stdout".to_string())?;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("read stdout: {e}"))?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return Err(error.to_string());
        }
        if value.get("pressed").and_then(|v| v.as_bool()) == Some(true) {
            match mode {
                TriggerMode::Toggle => {
                    let _ = tx.send(json!({ "event": "dictation_toggle_trigger" }));
                    let _ = tx.send(
                        json!({ "event": "dictation.trigger", "kind": "toggle", "phase": "down" }),
                    );
                }
                TriggerMode::PushToTalk => {
                    let _ = tx.send(json!({
                        "event": "dictation_ptt_trigger",
                        "phase": "down",
                    }));
                    let _ = tx.send(
                        json!({ "event": "dictation.trigger", "kind": "ptt", "phase": "down" }),
                    );
                }
            }
        }
        if value.get("released").and_then(|v| v.as_bool()) == Some(true) {
            if matches!(mode, TriggerMode::PushToTalk) {
                let _ = tx.send(json!({
                    "event": "dictation_ptt_trigger",
                    "phase": "up",
                }));
                let _ =
                    tx.send(json!({ "event": "dictation.trigger", "kind": "ptt", "phase": "up" }));
            }
            break;
        }
    }

    let _ = child.wait();
    child_done.store(true, Ordering::SeqCst);
    Ok(())
}

/// Запускает one-shot hotkey-capture: спавнит `capture-hotkey` helper, читает
/// первое `captured` / `cancelled` и эмитит в `tx` событие в том же формате,
/// что Windows-ветка (`dictation_capture_key` с готовым `accelerator` /
/// `dictation_capture_cancelled`). UI-слой остаётся платформо-агностичным.
pub fn begin_capture(tx: broadcast::Sender<Value>) -> Result<(), NativeHelperError> {
    let helper = resolve_helper("capture-hotkey")?;
    let generation = CAPTURE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    thread::spawn(move || {
        if let Err(error) = run_capture_once(&helper, &tx, generation) {
            // Любой сбой helper'а (нет permission, spawn fail) → отменяем
            // capture, чтобы UI не висел в состоянии «жду нажатие».
            let _ = error;
            let _ = tx.send(json!({ "event": "dictation_capture_cancelled" }));
        }
    });
    Ok(())
}

/// Останавливает активный capture (например юзер ушёл с поля / закрыл Settings
/// до нажатия). Бамп generation → watchdog SIGTERM'ит lingering helper.
pub fn end_capture() {
    CAPTURE_GENERATION.fetch_add(1, Ordering::SeqCst);
}

fn run_capture_once(
    helper: &Path,
    tx: &broadcast::Sender<Value>,
    generation: u64,
) -> Result<(), String> {
    let mut child = Command::new(helper)
        .env("MUNDUS_PARENT_PID", std::process::id().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let child_pid = child.id();
    let child_done = Arc::new(AtomicBool::new(false));
    {
        let child_done = child_done.clone();
        thread::spawn(move || {
            while !child_done.load(Ordering::SeqCst)
                && CAPTURE_GENERATION.load(Ordering::SeqCst) == generation
            {
                thread::sleep(std::time::Duration::from_millis(250));
            }
            if !child_done.load(Ordering::SeqCst) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(child_pid as libc::pid_t, libc::SIGTERM);
                }
            }
        });
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "missing helper stdout".to_string())?;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("read stdout: {e}"))?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            child_done.store(true, Ordering::SeqCst);
            let _ = child.wait();
            return Err(error.to_string());
        }
        if value.get("cancelled").and_then(|v| v.as_bool()) == Some(true) {
            let _ = tx.send(json!({ "event": "dictation_capture_cancelled" }));
            break;
        }
        if value.get("captured").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(accel) = capture_value_to_accelerator(&value) {
                let _ = tx.send(json!({
                    "event": "dictation_capture_key",
                    "accelerator": accel,
                }));
            }
            break;
        }
    }

    child_done.store(true, Ordering::SeqCst);
    let _ = child.wait();
    Ok(())
}

/// Конвертирует `captured`-payload helper'а в accelerator-строку. Возвращает
/// `None` если keyCode неизвестен — тогда capture молча игнорится (как
/// Windows-ветка при невалидной клавише).
pub(crate) fn capture_value_to_accelerator(value: &Value) -> Option<String> {
    // Дабл-тап Fn — особый хоткей "FnFn" (нет real keyCode-пары).
    if value.get("doubleFn").and_then(|v| v.as_bool()) == Some(true) {
        return Some("FnFn".into());
    }
    let key_code = value.get("keyCode").and_then(|v| v.as_u64())? as u16;
    let key = mac_key_name(key_code)?;
    let flag = |name: &str| value.get(name).and_then(|v| v.as_bool()).unwrap_or(false);
    Some(build_accelerator(
        key,
        flag("ctrl"),
        flag("alt"),
        flag("shift"),
        flag("cmd"),
        flag("fn"),
    ))
}

/// Собирает accelerator в том же порядке/нотации, что фронтовый
/// `buildAccelerator` (Ctrl+Alt+Shift+Super+Key) — Cmd мапится в `Super`, как
/// и Windows-side win. `parse_hotkey` и UI-display понимают этот формат.
fn build_accelerator(
    key: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    cmd: bool,
    function: bool,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if cmd {
        parts.push("Super");
    }
    if function {
        parts.push("Fn");
    }
    parts.push(key);
    parts.join("+")
}

/// Обратный маппинг к `mac_key_code`: CGKeyCode → каноничное имя клавиши в
/// формате accelerator'а (буквы в upper-case, как фронтовый `vkToKeyName`).
fn mac_key_name(code: u16) -> Option<&'static str> {
    let name = match code {
        0 => "A",
        1 => "S",
        2 => "D",
        3 => "F",
        4 => "H",
        5 => "G",
        6 => "Z",
        7 => "X",
        8 => "C",
        9 => "V",
        11 => "B",
        12 => "Q",
        13 => "W",
        14 => "E",
        15 => "R",
        16 => "Y",
        17 => "T",
        18 => "1",
        19 => "2",
        20 => "3",
        21 => "4",
        22 => "6",
        23 => "5",
        24 => "=",
        25 => "9",
        26 => "7",
        27 => "-",
        28 => "8",
        29 => "0",
        30 => "]",
        31 => "O",
        32 => "U",
        33 => "[",
        34 => "I",
        35 => "P",
        37 => "L",
        38 => "J",
        39 => "'",
        40 => "K",
        41 => ";",
        42 => "\\",
        43 => ",",
        44 => "/",
        45 => "N",
        46 => "M",
        47 => ".",
        49 => "Space",
        50 => "`",
        36 => "Enter",
        48 => "Tab",
        51 => "Backspace",
        117 => "Delete",
        115 => "Home",
        119 => "End",
        116 => "PageUp",
        121 => "PageDown",
        123 => "Left",
        124 => "Right",
        125 => "Down",
        126 => "Up",
        122 => "F1",
        120 => "F2",
        99 => "F3",
        118 => "F4",
        96 => "F5",
        97 => "F6",
        98 => "F7",
        100 => "F8",
        101 => "F9",
        109 => "F10",
        103 => "F11",
        111 => "F12",
        105 => "F13",
        107 => "F14",
        113 => "F15",
        106 => "F16",
        _ => return None,
    };
    Some(name)
}

fn bool_arg(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

#[derive(Debug, Clone)]
struct MacHotkeySpec {
    key_code: u16,
    cmd: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
    function: bool,
    /// "FnFn" — двойной тап по Fn (как системная диктовка Apple). На
    /// Windows нет Fn — там accelerator просто не парсится.
    double_fn: bool,
}

fn parse_hotkey(raw: &str) -> Option<MacHotkeySpec> {
    let mut spec = MacHotkeySpec {
        key_code: 0,
        cmd: false,
        ctrl: false,
        alt: false,
        shift: false,
        function: false,
        double_fn: false,
    };
    if raw.trim().eq_ignore_ascii_case("fnfn") || raw.trim() == "Fn ×2" {
        spec.key_code = 63; // kVK_Function
        spec.double_fn = true;
        return Some(spec);
    }
    let mut key: Option<&str> = None;
    for part in raw.split('+') {
        let token = part.trim().to_ascii_lowercase();
        match token.as_str() {
            "cmd" | "command" | "meta" | "super" => spec.cmd = true,
            "ctrl" | "control" => spec.ctrl = true,
            "alt" | "option" => spec.alt = true,
            "shift" => spec.shift = true,
            "fn" | "function" => spec.function = true,
            "" => {}
            _ => key = Some(part.trim()),
        }
    }
    spec.key_code = mac_key_code(key?)?;
    Some(spec)
}

fn mac_key_code(key: &str) -> Option<u16> {
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        "a" => Some(0),
        "s" => Some(1),
        "d" => Some(2),
        "f" => Some(3),
        "h" => Some(4),
        "g" => Some(5),
        "z" => Some(6),
        "x" => Some(7),
        "c" => Some(8),
        "v" => Some(9),
        "b" => Some(11),
        "q" => Some(12),
        "w" => Some(13),
        "e" => Some(14),
        "r" => Some(15),
        "y" => Some(16),
        "t" => Some(17),
        "1" => Some(18),
        "2" => Some(19),
        "3" => Some(20),
        "4" => Some(21),
        "6" => Some(22),
        "5" => Some(23),
        "=" => Some(24),
        "9" => Some(25),
        "7" => Some(26),
        "-" => Some(27),
        "8" => Some(28),
        "0" => Some(29),
        "]" => Some(30),
        "o" => Some(31),
        "u" => Some(32),
        "[" => Some(33),
        "i" => Some(34),
        "p" => Some(35),
        "l" => Some(37),
        "j" => Some(38),
        "'" => Some(39),
        "k" => Some(40),
        ";" | "semicolon" => Some(41),
        "\\" => Some(42),
        "," => Some(43),
        "/" => Some(44),
        "n" => Some(45),
        "m" => Some(46),
        "." => Some(47),
        "`" | "backquote" => Some(50),
        "space" => Some(49),
        "enter" | "return" => Some(36),
        "tab" => Some(48),
        "backspace" => Some(51),
        "delete" => Some(117),
        "home" => Some(115),
        "end" => Some(119),
        "pageup" => Some(116),
        "pagedown" => Some(121),
        "left" => Some(123),
        "right" => Some(124),
        "down" => Some(125),
        "up" => Some(126),
        "f1" => Some(122),
        "f2" => Some(120),
        "f3" => Some(99),
        "f4" => Some(118),
        "f5" => Some(96),
        "f6" => Some(97),
        "f7" => Some(98),
        "f8" => Some(100),
        "f9" => Some(101),
        "f10" => Some(109),
        "f11" => Some(103),
        "f12" => Some(111),
        "f13" => Some(105),
        "f14" => Some(107),
        "f15" => Some(113),
        "f16" => Some(106),
        _ => None,
    }
}

pub(crate) fn resolve_helper_pub(name: &str) -> Result<PathBuf, NativeHelperError> {
    resolve_helper(name)
}

fn resolve_helper(name: &str) -> Result<PathBuf, NativeHelperError> {
    let mut candidates = Vec::new();

    if let Ok(dir) = std::env::var("MUNDUS_MACOS_NATIVE_DIR") {
        push_candidate(&mut candidates, PathBuf::from(dir).join(name));
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            push_candidate(
                &mut candidates,
                parent.join("native").join("macos").join(name),
            );
            if let Some(resources) = parent.parent() {
                push_candidate(
                    &mut candidates,
                    resources.join("native").join("macos").join(name),
                );
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        push_candidate(
            &mut candidates,
            cwd.join("platform")
                .join("desktop")
                .join(".tmp")
                .join("native")
                .join("macos")
                .join(name),
        );
        push_candidate(
            &mut candidates,
            cwd.join(".tmp").join("native").join("macos").join(name),
        );
    }

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(NativeHelperError::NotFound {
        name: name.to_string(),
        candidates,
    })
}

fn push_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if !candidates.iter().any(|p| same_path(p, &path)) {
        candidates.push(path);
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}

#[cfg(test)]
#[path = "macos_native/tests.rs"]
mod tests;
