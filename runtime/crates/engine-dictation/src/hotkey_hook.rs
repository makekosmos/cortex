// Hotkey hook через Win32 WH_KEYBOARD_LL.
//
// Два use case'а:
//   1. Push-to-talk (hold-to-record) — нужны и down, и up event'ы;
//      `globalShortcut` шлёт только down → недостаточно.
//   2. Toggle через системные shortcut'ы (Win+H, Win+Space, etc.) — Electron
//      `globalShortcut.register` опирается на Win32 `RegisterHotKey`, который
//      возвращает MOD_ALREADY_REGISTERED для системных shortcut'ов
//      перехваченных explorer.exe. WH_KEYBOARD_LL — нижестоящий уровень,
//      получает событие ПЕРЕД shell'ом и может его **блокировать** через
//      возврат `LRESULT(1)`. Это даёт нам Win+H без отключения системного
//      Voice Typing в Settings.
//
// Решение: hook отслеживает заданное сочетание (accelerator parsed в `Matcher`)
// и emit'ит broadcast event'ы:
//   - PTT mode  → `dictation_ptt_trigger { phase: "down" | "up" }`
//   - Toggle mode → `dictation_toggle_trigger` (только на down)
// Когда matcher matches — событие consumed (intercept), Windows shell его не
// увидит. На non-match — propagation через `CallNextHookEx` (нормальный путь).
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
    GetAsyncKeyState, GetKeyboardLayout, SendInput, VkKeyScanExW, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU,
    VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, SetWindowsHookExW,
    UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matcher {
    /// Virtual-Key code основной клавиши (последняя в accelerator).
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Windows logo key (Super / Meta / Cmd) — Left или Right.
    pub win: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookMode {
    /// Hold-to-record. Emit и down, и up event ('dictation_ptt_trigger').
    PushToTalk,
    /// Toggle (как Electron globalShortcut). Emit только на down
    /// ('dictation_toggle_trigger'). Используется для accelerator'ов которые
    /// `RegisterHotKey` не может занять (системные Win+H, Win+Space).
    Toggle,
}

struct State {
    matcher: Option<Matcher>,
    sender: Option<broadcast::Sender<serde_json::Value>>,
    mode: HookMode,
    /// Текущее состояние "клавиша зажата" — для дедупа повторных WM_KEYDOWN
    /// (Windows шлёт повторы при удержании, нам нужен только первый down +
    /// один up).
    pressed: bool,
    /// Capture mode — Settings UI просит hook ловить СЛЕДУЮЩЕЕ non-modifier
    /// нажатие и emit'ить полный accelerator (vk + текущие modifiers).
    /// Это позволяет назначить системные shortcut'ы вроде Win+H — иначе
    /// Voice Typing срабатывает раньше WebContents keyboard handler'а.
    /// Hook intercept'ит событие, оригинальное действие не происходит.
    capture_active: bool,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static HOOK_THREAD_STARTED: OnceLock<()> = OnceLock::new();

fn state_mtx() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            matcher: None,
            sender: None,
            mode: HookMode::Toggle,
            pressed: false,
            capture_active: false,
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
                // Capture mode имеет приоритет над обычным matcher'ом — пока
                // Settings UI ждёт назначения hotkey'я, ВСЕ нажатия идут к нам.
                if handle_capture(info.vkCode, is_down) {
                    return LRESULT(1);
                }
                if handle_event(info.vkCode, is_down) {
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

/// Returns `true` если capture mode активен И событие было captured/intercepted.
/// На non-modifier keydown эмитит accelerator. Modifier нажатия ВСЕГДА
/// пропускаются через `CallNextHookEx` — иначе Windows не зарегистрирует их
/// в keyboard state, и `GetAsyncKeyState` для VK_LWIN/VK_CONTROL/etc. вернёт
/// false, когда мы захотим проверить модификаторы при non-modifier нажатии.
///
/// Системный shortcut (Win+H) при этом всё равно не сработает: WM_HOTKEY для
/// `RegisterHotKey` генерируется ПОСЛЕ keyboard hook chain'а — если мы
/// intercept'ним именно нажатие H (через return LRESULT(1)), WM_HOTKEY для
/// Win+H не дойдёт до Voice Typing.
fn handle_capture(vk: u32, is_down: bool) -> bool {
    let guard = match state_mtx().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if !guard.capture_active {
        return false;
    }
    let sender_opt = guard.sender.clone();
    drop(guard);

    // Modifier — пропускаем дальше (см. doc-comment выше), не intercept.
    if is_modifier_vk(vk) {
        return false;
    }

    // ESC отменяет capture (не emit'им accelerator).
    if is_down && vk == 0x1B {
        if let Some(tx) = sender_opt.as_ref() {
            let _ = tx.send(json!({
                "event": "dictation_capture_cancelled",
            }));
        }
        // Auto-deactivate — host.rs не нужен round trip.
        if let Ok(mut g) = state_mtx().lock() {
            g.capture_active = false;
        }
        return true;
    }

    // Non-modifier — собираем accelerator из текущего state модификаторов.
    // Эмитим только если хотя бы один modifier зажат (one-key hotkey'и
    // запрещены на Windows: одиночная буква = обычный input, не shortcut).
    if is_down {
        let ctrl = check_mod(true, VK_CONTROL.0);
        let shift = check_mod(true, VK_SHIFT.0);
        let alt = check_mod(true, VK_MENU.0);
        let win = check_mod_win(true);

        // Без модификатора — игнор (но keyup всё равно intercept'ится, чтобы
        // системный shortcut не отработал на retry'е).
        if !ctrl && !shift && !alt && !win {
            // Не intercept — пусть символ дойдёт до того окна где юзер
            // случайно набрал букву (он же не закрывал Settings).
            return false;
        }

        if let Some(tx) = sender_opt {
            let _ = tx.send(json!({
                "event": "dictation_capture_key",
                "vk": vk,
                "ctrl": ctrl,
                "shift": shift,
                "alt": alt,
                "win": win,
            }));
        }
        // Auto-deactivate — одно валидное нажатие = одна capture session.
        if let Ok(mut g) = state_mtx().lock() {
            g.capture_active = false;
        }
        // Подавить Start menu trigger от Win-up без другой клавиши.
        swallow_win_shortcut_if_active();
    }
    true
}

/// Если Win key зажат и мы только что intercept'или акорд — асинхронно
/// (в отдельном thread'е) посылаем **dummy** SendInput с VK_NONAME (0xFC).
/// Это не имеет визуального эффекта, но Windows считает что Win key был
/// использован как modifier (не "lonely press") → при отпускании Win-up
/// Start menu НЕ откроется.
///
/// КРИТИЧНО: SendInput вызывается из spawned thread, НЕ из hook callback'а.
/// Если делать sync внутри callback'а:
///   1. SendInput генерирует input event, который проходит через ту же
///      hook chain ещё раз → рекурсия → timing-чувствительный race.
///   2. Hook callback должен возвращаться < ~300ms (LowLevelHooksTimeout),
///      иначе Windows временно деактивирует hook → системный shortcut
///      пробивается (Voice Typing / audio picker открывается).
fn swallow_win_shortcut_if_active() {
    if !check_mod_win(true) {
        return;
    }
    thread::spawn(|| {
        // Dummy down + up для VK_NONAME (0xFC). Не привязан ни к одной
        // реальной клавише, но идёт через системный input pipeline и
        // засчитывается как "key with modifier" → отменяет Start menu trigger.
        let dummy_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0xFC),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let dummy_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0xFC),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [dummy_down, dummy_up];
        let cb = std::mem::size_of::<INPUT>() as i32;
        unsafe {
            SendInput(&inputs, cb);
        }
    });
}

fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x10 | // VK_SHIFT
        0xA0 | // VK_LSHIFT
        0xA1 | // VK_RSHIFT
        0x11 | // VK_CONTROL
        0xA2 | // VK_LCONTROL
        0xA3 | // VK_RCONTROL
        0x12 | // VK_MENU (Alt)
        0xA4 | // VK_LMENU
        0xA5 | // VK_RMENU
        0x5B | // VK_LWIN
        0x5C // VK_RWIN
    )
}

/// Возвращает `true` если событие consumed (intercept). Включает logging
/// в broadcast при match + дедуп auto-repeat'а.
fn handle_event(vk: u32, is_down: bool) -> bool {
    // Атомарно: проверка дедупа + update pressed-flag + snapshot для emit.
    // КРИТИЧНО: pressed-flag ставится ПОД ТЕМ ЖЕ LOCK'ом до emit'а — иначе
    // auto-repeat от удержания клавиши (Windows шлёт WM_KEYDOWN каждые
    // ~33ms) проскакивает через флаг пока мы выполняли emit без lock'а →
    // toggleDictation вызывался 6 раз на одно нажатие.
    let (mode, sender_opt) = {
        let mut guard = match state_mtx().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let Some(m) = guard.matcher else {
            return false;
        };
        if !main_key_matches(m.vk, vk) {
            return false;
        }
        if is_down && guard.pressed {
            // Auto-repeat от удержания — игнор события. Intercept (return true)
            // чтобы системный shortcut не сработал на втором tick'е.
            return true;
        }
        let mods_ok = check_mod(m.ctrl, VK_CONTROL.0)
            && check_mod(m.shift, VK_SHIFT.0)
            && check_mod(m.alt, VK_MENU.0)
            && check_mod_win(m.win);
        if !mods_ok {
            return false;
        }
        // ВАЖНО: ставим pressed=is_down ДО emit'а, под тем же lock'ом, который
        // проверял дедуп — соседний WM_KEYDOWN не сможет проскочить.
        guard.pressed = is_down;
        (guard.mode, guard.sender.clone())
    };

    // Emit event соответственно режиму (вне lock'а, чтобы broadcast не
    // блокировал hook callback'у нашему же thread'у).
    match mode {
        HookMode::PushToTalk => {
            let phase = if is_down { "down" } else { "up" };
            if let Some(tx) = sender_opt {
                let _ = tx.send(json!({
                    "event": "dictation_ptt_trigger",
                    "phase": phase,
                }));
                let _ = tx.send(json!({
                    "event": "dictation.trigger",
                    "kind": "ptt",
                    "phase": phase,
                }));
            }
        }
        HookMode::Toggle => {
            if is_down {
                if let Some(tx) = sender_opt {
                    let _ = tx.send(json!({
                        "event": "dictation_toggle_trigger",
                    }));
                    let _ = tx.send(json!({
                        "event": "dictation.trigger",
                        "kind": "toggle",
                        "phase": "down",
                    }));
                }
            }
        }
    }

    // Подавить Start menu trigger при intercept'е Win-shortcut'а
    // (см. swallow_win_shortcut_if_active).
    if is_down {
        swallow_win_shortcut_if_active();
    }
    true
}

fn main_key_matches(expected_vk: u32, event_vk: u32) -> bool {
    if expected_vk == event_vk {
        return true;
    }
    layout_key_matches(
        event_vk,
        oem_symbol(expected_vk).and_then(foreground_layout_vk),
    )
}

fn layout_key_matches(event_vk: u32, layout_vk: Option<u32>) -> bool {
    layout_vk == Some(event_vk)
}

fn oem_symbol(vk: u32) -> Option<char> {
    match vk {
        0xBA => Some(';'),
        0xBF => Some('/'),
        0xC0 => Some('`'),
        0xDB => Some('['),
        0xDC => Some('\\'),
        0xDD => Some(']'),
        0xDE => Some('\''),
        0xBC => Some(','),
        0xBE => Some('.'),
        0xBD => Some('-'),
        0xBB => Some('='),
        _ => None,
    }
}

fn foreground_layout_vk(symbol: char) -> Option<u32> {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return None;
        }
        let thread_id = GetWindowThreadProcessId(foreground, None);
        let mapped = VkKeyScanExW(symbol as u16, GetKeyboardLayout(thread_id));
        (mapped != -1).then_some((mapped as u16 & 0xff) as u32)
    }
}

fn check_mod(required: bool, vk: u16) -> bool {
    if !required {
        return true;
    }
    let state = unsafe { GetAsyncKeyState(vk as i32) };
    (state as u16 & 0x8000) != 0
}

/// Win key: проверяем оба — Left (VK_LWIN, 0x5B) и Right (VK_RWIN, 0x5C).
fn check_mod_win(required: bool) -> bool {
    if !required {
        return true;
    }
    unsafe {
        let l = GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000;
        let r = GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000;
        (l | r) != 0
    }
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

/// Включает / выключает capture mode. Когда active, hook intercept'ит ВСЕ
/// keystrokes (включая модификаторы), на первое non-modifier нажатие emit'ит
/// `dictation_capture_key` event с accelerator'ом и автоматически
/// деактивируется. ESC — `dictation_capture_cancelled`. Sender используется
/// общий с regular hotkey hook'ом (тот же broadcast).
pub fn set_capture_mode(active: bool, sender: Option<broadcast::Sender<serde_json::Value>>) {
    ensure_thread_started();
    let mut g = match state_mtx().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g.capture_active = active;
    if active && sender.is_some() {
        g.sender = sender;
    }
}

/// Активирует hook'у matcher + sender + mode. None для matcher = выключает emit
/// (hook продолжает крутиться вхолостую). Можно вызвать многократно при
/// смене hotkey'я / trigger_mode.
pub fn set_active(
    matcher: Option<Matcher>,
    sender: Option<broadcast::Sender<serde_json::Value>>,
    mode: HookMode,
) {
    ensure_thread_started();
    let mut g = match state_mtx().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g.matcher = matcher;
    g.sender = sender;
    g.mode = mode;
    g.pressed = false;
}

/// Parse Electron-style accelerator string в Matcher. Поддерживает
/// модификаторы `Ctrl`, `Shift`, `Alt`, и базовый набор клавиш:
/// A-Z, 0-9, F1-F12, common punctuation. Возвращает None если не распарсилось.
pub fn parse_accelerator(s: &str) -> Option<Matcher> {
    let parts: Vec<&str> = s
        .split('+')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    let mut vk: Option<u32> = None;
    for part in &parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "super" | "meta" | "cmd" | "command" | "win" | "windows" => win = true,
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
        win,
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
            ';' | ':' => Some(0xBA),  // VK_OEM_1
            '/' | '?' => Some(0xBF),  // VK_OEM_2
            '`' | '~' => Some(0xC0),  // VK_OEM_3
            '[' | '{' => Some(0xDB),  // VK_OEM_4
            '\\' | '|' => Some(0xDC), // VK_OEM_5
            ']' | '}' => Some(0xDD),  // VK_OEM_6
            '\'' | '"' => Some(0xDE), // VK_OEM_7
            ',' | '<' => Some(0xBC),  // VK_OEM_COMMA
            '.' | '>' => Some(0xBE),  // VK_OEM_PERIOD
            '-' | '_' => Some(0xBD),  // VK_OEM_MINUS
            '=' | '+' => Some(0xBB),  // VK_OEM_PLUS
            _ => None,
        };
    }
    // F1-F24
    if let Some(rest) = key.strip_prefix('F').or_else(|| key.strip_prefix('f')) {
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
    fn oem_symbols_can_match_their_current_layout_virtual_key() {
        assert_eq!(oem_symbol(0xBA), Some(';'));
        assert!(main_key_matches(0xBA, 0xBA));
    }

    #[test]
    fn layout_virtual_key_is_accepted_for_oem_symbol() {
        assert!(layout_key_matches(0x34, Some(0x34)));
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
    fn parse_super_d() {
        let m = parse_accelerator("Super+D").expect("parse ok");
        assert_eq!(m.vk, 'D' as u32);
        assert!(m.win);
        assert!(!m.ctrl);
    }

    #[test]
    fn parse_win_h_alias() {
        // "Win+H" — алиас "Super+H" из HotkeyCapture / Electron-style.
        let m = parse_accelerator("Win+H").expect("parse ok");
        assert_eq!(m.vk, 'H' as u32);
        assert!(m.win);
    }

    #[test]
    fn parse_ctrl_shift_super_h() {
        let m = parse_accelerator("Ctrl+Shift+Super+H").expect("parse ok");
        assert_eq!(m.vk, 'H' as u32);
        assert!(m.ctrl);
        assert!(m.shift);
        assert!(m.win);
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
        set_active(None, None, HookMode::Toggle);
        set_active(
            Some(Matcher {
                vk: 0x41,
                ctrl: false,
                shift: false,
                alt: false,
                win: false,
            }),
            None,
            HookMode::Toggle,
        );
        set_active(None, None, HookMode::PushToTalk);
        set_active(None, None, HookMode::Toggle);
    }
}
