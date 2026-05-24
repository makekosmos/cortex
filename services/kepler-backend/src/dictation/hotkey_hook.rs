// Push-to-talk через Win32 low-level keyboard hook.
//
// Electron `globalShortcut` шлёт только key-down event'ы — для PTT (hold-to-record)
// этого недостаточно. Решение: ставим WH_KEYBOARD_LL hook в выделенном OS-потоке
// с message-pump'ом. Hook отслеживает заданное сочетание (accelerator parsed
// в `Matcher`) и emit'ит broadcast events `dictation_ptt_trigger { phase }`
// — Electron слушает и вызывает `toggleDictation()` (на down → старт записи,
// на up → стоп + submit, та же семантика что toggle, два вызова).
//
// Безопасность производительности hook'а: callback `keyboard_proc` ОБЯЗАН
// возвращаться быстро (иначе Windows блокирует все клавишные события).
// Делаем только: разыменование KBDLLHOOKSTRUCT, GetAsyncKeyState на модификаторы,
// `broadcast::Sender::send` (non-blocking). Никаких блокирующих ops, lock'ов
// долгих секций, allocation'ов.
//
// Threading: hook callback вызывается в hook-потоке. Этот же поток крутит
// `GetMessageW` loop — Windows требует чтобы hook жил на thread'е с
// message-pump'ом. Замена hotkey идёт через atomic swap'ы в `MATCHER`/`SENDER`.
//
// Linux/macOS: модуль no-op'ит (stub функции). PTT в Phase 1 — Windows-only.

#![cfg(windows)]
#![allow(unsafe_code)]

use std::sync::{Mutex, OnceLock};
use std::thread;

use serde_json::json;
use tokio::sync::broadcast;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, HHOOK,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matcher {
    /// Virtual-Key code основной клавиши (последняя в accelerator).
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

struct State {
    matcher: Option<Matcher>,
    sender: Option<broadcast::Sender<serde_json::Value>>,
    /// Текущее состояние "клавиша зажата" — для дедупа повторных WM_KEYDOWN
    /// (Windows шлёт повторы при удержании, нам нужен только первый down +
    /// один up).
    pressed: bool,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static HOOK_THREAD_STARTED: OnceLock<()> = OnceLock::new();

fn state_mtx() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            matcher: None,
            sender: None,
            pressed: false,
        })
    })
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info_ptr = lparam.0 as *const KBDLLHOOKSTRUCT;
        if !info_ptr.is_null() {
            let info = &*info_ptr;
            let msg = wparam.0 as u32;
            let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            if is_down || is_up {
                handle_event(info.vkCode, is_down);
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

fn handle_event(vk: u32, is_down: bool) {
    // Lock короткий — только snapshot + send. Если lock poisoned —
    // recover через into_inner (poison из panic в predшествующей секции;
    // sender/matcher остаются valid).
    let guard = match state_mtx().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let Some(m) = guard.matcher else {
        return;
    };
    if vk != m.vk {
        return;
    }
    if is_down && guard.pressed {
        // Auto-repeat от удержания — игнор.
        return;
    }
    let mods_ok =
        check_mod(m.ctrl, VK_CONTROL.0) && check_mod(m.shift, VK_SHIFT.0) && check_mod(m.alt, VK_MENU.0);
    if !mods_ok {
        return;
    }
    let phase = if is_down { "down" } else { "up" };
    if let Some(tx) = guard.sender.as_ref() {
        let _ = tx.send(json!({
            "event": "dictation_ptt_trigger",
            "phase": phase,
        }));
    }
    // Update pressed-flag для дедупа.
    drop(guard);
    if let Ok(mut g) = state_mtx().lock() {
        g.pressed = is_down;
    }
}

fn check_mod(required: bool, vk: u16) -> bool {
    if !required {
        return true;
    }
    let state = unsafe { GetAsyncKeyState(vk as i32) };
    (state as u16 & 0x8000) != 0
}

/// Запускает hook-thread один раз (idempotent). После старта thread живёт
/// до завершения процесса.
fn ensure_thread_started() {
    HOOK_THREAD_STARTED.get_or_init(|| {
        thread::spawn(|| {
            unsafe {
                let hook =
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HINSTANCE::default(), 0);
                let hook = match hook {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("[dictation::hotkey_hook] SetWindowsHookExW failed: {e}");
                        return;
                    }
                };
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    // No DispatchMessageW: у нас нет window, callbacks из hook'а
                    // приходят в этот же thread автоматически через SendMessage'ы Windows.
                }
                let _ = UnhookWindowsHookEx(hook);
            }
        });
    });
}

/// Активирует hook'у matcher + sender. None для matcher = выключает emit
/// (hook продолжает крутиться вхолостую). Можно вызвать многократно при
/// смене hotkey'я / trigger_mode.
pub fn set_active(matcher: Option<Matcher>, sender: Option<broadcast::Sender<serde_json::Value>>) {
    ensure_thread_started();
    let mut g = match state_mtx().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g.matcher = matcher;
    g.sender = sender;
    g.pressed = false;
}

/// Parse Electron-style accelerator string в Matcher. Поддерживает
/// модификаторы `Ctrl`, `Shift`, `Alt`, и базовый набор клавиш:
/// A-Z, 0-9, F1-F12, common punctuation. Возвращает None если не распарсилось.
pub fn parse_accelerator(s: &str) -> Option<Matcher> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut vk: Option<u32> = None;
    for part in &parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "super" | "meta" | "cmd" | "command" => {
                // Windows logo key — пропускаем как unsupported в Phase 1.5.
                return None;
            }
            key => {
                vk = key_to_vk(key);
            }
        }
    }
    vk.map(|v| Matcher {
        vk: v,
        ctrl,
        shift,
        alt,
    })
}

fn key_to_vk(key: &str) -> Option<u32> {
    // Single ASCII letter A-Z → 0x41-0x5A
    if key.len() == 1 {
        let c = key.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_alphabetic() {
            return Some(c as u32);
        }
        if c.is_ascii_digit() {
            return Some(c as u32);
        }
        // Common punctuation → VK_OEM_*
        return match c {
            ';' | ':' => Some(0xBA), // VK_OEM_1
            '/' | '?' => Some(0xBF), // VK_OEM_2
            '`' | '~' => Some(0xC0), // VK_OEM_3
            '[' | '{' => Some(0xDB), // VK_OEM_4
            '\\' | '|' => Some(0xDC), // VK_OEM_5
            ']' | '}' => Some(0xDD), // VK_OEM_6
            '\'' | '"' => Some(0xDE), // VK_OEM_7
            ',' | '<' => Some(0xBC), // VK_OEM_COMMA
            '.' | '>' => Some(0xBE), // VK_OEM_PERIOD
            '-' | '_' => Some(0xBD), // VK_OEM_MINUS
            '=' | '+' => Some(0xBB), // VK_OEM_PLUS
            _ => None,
        };
    }
    // F1-F24
    if let Some(rest) = key
        .strip_prefix('F')
        .or_else(|| key.strip_prefix('f'))
    {
        if let Ok(n) = rest.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Some(0x70 + (n - 1));
            }
        }
    }
    // Named keys
    match key.to_ascii_lowercase().as_str() {
        "space" | "spacebar" => Some(0x20),
        "tab" => Some(0x09),
        "enter" | "return" => Some(0x0D),
        "escape" | "esc" => Some(0x1B),
        "backspace" => Some(0x08),
        "delete" | "del" => Some(0x2E),
        "insert" | "ins" => Some(0x2D),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        "left" => Some(0x25),
        "up" => Some(0x26),
        "right" => Some(0x27),
        "down" => Some(0x28),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_shift_semicolon() {
        let m = parse_accelerator("Ctrl+Shift+;").expect("parse ok");
        assert_eq!(m.vk, 0xBA);
        assert!(m.ctrl);
        assert!(m.shift);
        assert!(!m.alt);
    }

    #[test]
    fn parse_alt_d() {
        let m = parse_accelerator("Alt+D").expect("parse ok");
        assert_eq!(m.vk, 'D' as u32);
        assert!(!m.ctrl);
        assert!(m.alt);
    }

    #[test]
    fn parse_f12() {
        let m = parse_accelerator("F12").expect("parse ok");
        assert_eq!(m.vk, 0x7B);
        assert!(!m.ctrl);
    }

    #[test]
    fn parse_ctrl_alt_space() {
        let m = parse_accelerator("Ctrl+Alt+Space").expect("parse ok");
        assert_eq!(m.vk, 0x20);
        assert!(m.ctrl);
        assert!(m.alt);
    }

    #[test]
    fn parse_super_unsupported() {
        assert!(parse_accelerator("Super+D").is_none());
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_accelerator("").is_none());
        assert!(parse_accelerator("Ctrl+").is_none());
    }

    #[test]
    fn parse_cmdorctrl_alias() {
        let m = parse_accelerator("CommandOrControl+P").expect("parse ok");
        assert_eq!(m.vk, 'P' as u32);
        assert!(m.ctrl);
    }

    #[test]
    fn set_active_idempotent_with_none() {
        // Не должно паниковать при отсутствии sender — ставим/снимаем matcher.
        set_active(None, None);
        set_active(
            Some(Matcher {
                vk: 0x41,
                ctrl: false,
                shift: false,
                alt: false,
            }),
            None,
        );
        set_active(None, None);
    }
}
