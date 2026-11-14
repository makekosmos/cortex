//! The global hotkey, heard by this process.
//!
//! macOS grants Input Monitoring to the program the person toggles in System
//! Settings. That program is Mundus Manager, so the event tap lives here, not
//! in a helper binary: a helper carries its own signature and never sees the
//! key. The Engine still owns capture, transcription and insertion; this file
//! only says «start» or «stop», and reads the keys the person presses while
//! the settings page waits for a combination.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

use imago_nsh::ComponentContext;

/// The page that mirrors the hotkey's state, once a page exists.
static PAGE: Mutex<Option<ComponentContext>> = Mutex::new(None);
/// The tap is listening.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// macOS allows this program to watch the keyboard.
static GRANTED: AtomicBool = AtomicBool::new(false);
/// The settings page waits for a combination.
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// A modifier is held down, for the widget's keycaps.
static HELD: [AtomicBool; 5] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];
/// The moment of the last Fn press, for the double tap.
#[cfg(target_os = "macos")]
static LAST_FN: AtomicU64 = AtomicU64::new(0);
/// Which wait for a combination is the current one, so an old timeout does
/// not cancel a new attempt.
static ATTEMPT: AtomicU64 = AtomicU64::new(0);
/// The combination the settings hold, as `dictation-config.json` has it.
#[cfg(target_os = "macos")]
static TRIGGER: Mutex<String> = Mutex::new(String::new());

pub fn start() {
    // The Manager keeps the tap, so the Engine's helper must go at once: it
    // carries its own signature, cannot see the key, and would fire a second
    // toggle if it ever did. Windows has no Manager tap yet, so there the
    // Engine must keep the hook — releasing it would leave the key dead for
    // as long as the window is open.
    #[cfg(target_os = "macos")]
    {
        crate::handover::from_engine();
        macos::start();
    }
}

/// The page that should hear what the tap sees. Called once, when the pages
/// create the bridge; the tap itself starts from `main`.
pub fn attach(context: ComponentContext) {
    if let Ok(mut page) = PAGE.lock() {
        *page = Some(context);
    }
}

/// Whether the hotkey can fire, and whether the page is waiting for a key.
pub fn status() -> Value {
    json!({
        "running": RUNNING.load(Ordering::SeqCst),
        "listenAccess": GRANTED.load(Ordering::SeqCst),
        "capturing": CAPTURING.load(Ordering::SeqCst),
        "held": held_keys(),
    })
}

/// The settings page asks for a combination: the next press becomes the value.
pub fn arm() {
    HELD.iter()
        .for_each(|held| held.store(false, Ordering::SeqCst));
    CAPTURING.store(true, Ordering::SeqCst);
    // Windows has no tap to read the combination, so a poller stands in for it.
    #[cfg(target_os = "windows")]
    windows::watch();
    log("capture armed");
    let attempt = ATTEMPT.fetch_add(1, Ordering::SeqCst) + 1;
    publish();
    // A person who walks away leaves the page waiting forever, and the next
    // key pressed anywhere in the system would become the hotkey.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if ATTEMPT.load(Ordering::SeqCst) == attempt && CAPTURING.load(Ordering::SeqCst) {
            cancel("timeout");
        }
    });
}

/// Asks macOS again, so the app's row comes back into «Мониторинг ввода» after
/// a reset or a rename. The dialog shows once; the row is what matters.
pub fn request_access() {
    #[cfg(target_os = "macos")]
    {
        unsafe { macos::request_listen_access() };
        publish();
    }
}

pub fn disarm() {
    if CAPTURING.swap(false, Ordering::SeqCst) {
        publish();
    }
}

/// Windows capture: macOS gets the combination from its event tap, Windows has
/// none, so the keys are polled instead. `GetAsyncKeyState` reports the state of
/// every key without consuming anything, which is what a "press the combination
/// you want" prompt needs — the page keeps its own text input working.
#[cfg(target_os = "windows")]
mod windows {
    use std::sync::atomic::Ordering;
    use std::sync::Once;

    use super::{cancel, finish, log, publish, CAPTURING};

    /// `GetAsyncKeyState`'s high bit: the key is down at this moment.
    const DOWN: i16 = 0x8000u16 as i16;

    const CTRL: [u32; 3] = [0x11, 0xA2, 0xA3];
    const SHIFT: [u32; 3] = [0x10, 0xA0, 0xA1];
    const ALT: [u32; 3] = [0x12, 0xA4, 0xA5];
    const WIN: [u32; 2] = [0x5B, 0x5C];

    /// The names the Engine parses back (`hotkey_hook::key_to_vk`): anything
    /// outside this table would be captured and then silently ignored.
    const KEYS: &[(u32, &str)] = &[
        (0x08, "Backspace"),
        (0x09, "Tab"),
        (0x0D, "Enter"),
        (0x20, "Space"),
        (0x21, "PageUp"),
        (0x22, "PageDown"),
        (0x23, "End"),
        (0x24, "Home"),
        (0x25, "Left"),
        (0x26, "Up"),
        (0x27, "Right"),
        (0x28, "Down"),
        (0x2D, "Insert"),
        (0x2E, "Delete"),
        (0xBA, ";"),
        (0xBB, "="),
        (0xBC, ","),
        (0xBD, "-"),
        (0xBE, "."),
        (0xBF, "/"),
        (0xC0, "`"),
        (0xDB, "["),
        (0xDC, "\\"),
        (0xDD, "]"),
        (0xDE, "'"),
    ];

    /// Starts the poller once; it idles between captures.
    pub(super) fn watch() {
        static STARTED: Once = Once::new();
        STARTED.call_once(|| {
            std::thread::spawn(run);
        });
    }

    fn run() {
        let mut previous: Vec<u32> = Vec::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(40));
            if !CAPTURING.load(Ordering::SeqCst) {
                previous.clear();
                continue;
            }
            let mut down = pressed();
            let fresh: Vec<u32> = down
                .iter()
                .copied()
                .filter(|vk| !previous.contains(vk))
                .collect();
            previous = std::mem::take(&mut down);
            let Some(&first) = fresh.first() else {
                continue;
            };
            if first == 0x1B && modifiers().is_empty() {
                log("capture cancelled");
                cancel("escape");
                continue;
            }
            let held = modifiers();
            if held.is_empty() {
                // A bare key is a person typing; the combination needs a
                // modifier, and the Engine refuses one-key hotkeys anyway.
                publish();
                continue;
            }
            let Some(token) = token_of(first) else {
                publish();
                continue;
            };
            let text = format!("{}+{token}", held.join("+"));
            log(&format!("captured {text}"));
            finish(&text);
        }
    }

    /// Every tracked key that is down right now.
    fn pressed() -> Vec<u32> {
        let mut keys = Vec::new();
        for (vk, _) in KEYS {
            if is_down(*vk) {
                keys.push(*vk);
            }
        }
        for vk in 0x30..=0x39u32 {
            if is_down(vk) {
                keys.push(vk);
            }
        }
        for vk in 0x41..=0x5Au32 {
            if is_down(vk) {
                keys.push(vk);
            }
        }
        for vk in 0x70..=0x87u32 {
            if is_down(vk) {
                keys.push(vk);
            }
        }
        keys
    }

    fn token_of(vk: u32) -> Option<String> {
        if let Some((_, token)) = KEYS.iter().find(|(key, _)| *key == vk) {
            return Some((*token).to_owned());
        }
        if (0x30..=0x39).contains(&vk) {
            return Some(char::from_u32(vk)?.to_string());
        }
        if (0x41..=0x5A).contains(&vk) {
            return Some(char::from_u32(vk)?.to_string());
        }
        if (0x70..=0x87).contains(&vk) {
            return Some(format!("F{}", vk - 0x6F));
        }
        None
    }

    fn modifiers() -> Vec<&'static str> {
        let mut held = Vec::new();
        if CTRL.iter().any(|vk| is_down(*vk)) {
            held.push("Ctrl");
        }
        if SHIFT.iter().any(|vk| is_down(*vk)) {
            held.push("Shift");
        }
        if ALT.iter().any(|vk| is_down(*vk)) {
            held.push("Alt");
        }
        if WIN.iter().any(|vk| is_down(*vk)) {
            held.push("Win");
        }
        held
    }

    fn is_down(vk: u32) -> bool {
        unsafe { GetAsyncKeyState(vk as i32) & DOWN != 0 }
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
}

/// The combination the person set, read again while the app runs: the page
/// saves it through the Engine, and the tap has to follow within a second.
#[cfg(target_os = "macos")]
fn trigger() -> String {
    TRIGGER
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn refresh_trigger() {
    let Some(dir) = crate::engine::data_dir() else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(dir.join("dictation-config.json")) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let hotkey = value
        .get("hotkey")
        .and_then(|field| field.as_str())
        .unwrap_or_default()
        .to_owned();
    if let Ok(mut slot) = TRIGGER.lock() {
        *slot = hotkey;
    }
}

fn held_keys() -> Vec<&'static str> {
    const NAMES: [&str; 5] = ["ctrl", "alt", "shift", "cmd", "fn"];
    NAMES
        .iter()
        .enumerate()
        .filter(|(index, _)| HELD[*index].load(Ordering::SeqCst))
        .map(|(_, name)| *name)
        .collect()
}

fn publish() {
    let event = json!({
        "capturing": CAPTURING.load(Ordering::SeqCst),
        "held": held_keys(),
    });
    emit("hotkeyState", event);
}

fn finish(accelerator: &str) {
    ATTEMPT.fetch_add(1, Ordering::SeqCst);
    CAPTURING.store(false, Ordering::SeqCst);
    HELD.iter()
        .for_each(|held| held.store(false, Ordering::SeqCst));
    emit("hotkey", json!({ "accelerator": accelerator }));
    publish();
}

fn cancel(reason: &str) {
    ATTEMPT.fetch_add(1, Ordering::SeqCst);
    CAPTURING.store(false, Ordering::SeqCst);
    HELD.iter()
        .for_each(|held| held.store(false, Ordering::SeqCst));
    emit("hotkey", json!({ "cancelled": true, "reason": reason }));
    publish();
}

fn emit(name: &str, data: Value) {
    if let Ok(page) = PAGE.lock() {
        if let Some(context) = page.as_ref() {
            context.emit(name, data);
        }
    }
}

/// Asks the Engine to start or stop the dictation, on a thread of its own: the
/// tap's callback must answer at once or macOS disables it.
#[cfg(target_os = "macos")]
fn toggle() {
    std::thread::spawn(|| {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let _ = engine.rpc("dictation.toggle", json!({}));
    });
}

/// Whether this process is hearing the keyboard.
pub fn listening() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Notes what the tap does, for a person who cannot see a key do anything:
/// `~/Library/Application Support/Mundus/logs/hotkey.log`.
pub fn log(line: &str) {
    crate::log::append("hotkey", line);
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{HELD, LAST_FN};

    const KEY_DOWN: u32 = 10;
    const KEY_UP: u32 = 11;
    const FLAGS_CHANGED: u32 = 12;
    const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    const TAP_DISABLED_BY_INPUT: u32 = 0xFFFF_FFFF;

    const FN_KEYCODE: i64 = 63;
    const ESC_KEYCODE: i64 = 53;
    const DOUBLE_MS: u64 = 800;

    const SHIFT: u64 = 1 << 17;
    const CONTROL: u64 = 1 << 18;
    const OPTION: u64 = 1 << 19;
    const COMMAND: u64 = 1 << 20;
    const SECONDARY_FN: u64 = 1 << 23;

    static mut TAP: *mut c_void = std::ptr::null_mut();
    /// The Fn key was down, so a repeated flags event is not a new press.
    static FN_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub unsafe fn request_listen_access() {
        CGRequestListenEventAccess();
    }

    pub fn start() {
        std::thread::spawn(|| {
            if let Err(error) = run() {
                super::log(&format!("tap stopped: {error}"));
                eprintln!("hotkey: {error}");
            }
        });
    }

    fn run() -> Result<(), String> {
        // Two grants can let this process hear the keyboard: Input Monitoring
        // (a listening tap) and Accessibility (an active tap). macOS keeps them
        // apart, so either one is enough and the person may already have given
        // Accessibility for pasting.
        let mut listening = false;
        let mut asked: Option<Instant> = None;
        let mut told = None;
        loop {
            super::refresh_trigger();
            let listen = unsafe { CGPreflightListenEventAccess() };
            let active = unsafe { AXIsProcessTrusted() };
            let granted = listen || active;
            // The list in System Settings holds only apps that asked, and a
            // reset or a rename drops the row. Asking again keeps the app
            // reachable; macOS shows its dialog once.
            if !granted && asked.is_none_or(|at| at.elapsed() > Duration::from_secs(10)) {
                asked = Some(Instant::now());
                unsafe { CGRequestListenEventAccess() };
            }
            super::GRANTED.store(granted, Ordering::SeqCst);
            if told != Some(granted) {
                told = Some(granted);
                super::log(&format!(
                    "granted: {granted} (monitoring: {listen}, accessibility: {active})"
                ));
                super::publish();
            }
            if !listening && granted {
                let options = if listen { 1 } else { 0 };
                let tap = unsafe {
                    CGEventTapCreate(
                        0,
                        0,
                        options,
                        (1 << FLAGS_CHANGED) | (1 << KEY_DOWN) | (1 << KEY_UP),
                        on_event,
                        std::ptr::null_mut(),
                    )
                };
                if tap.is_null() {
                    super::log("tap not created");
                } else {
                    unsafe {
                        TAP = tap;
                        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                        if source.is_null() {
                            return Err("run loop source was not created".into());
                        }
                        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
                        CGEventTapEnable(tap, true);
                    }
                    listening = true;
                    super::RUNNING.store(true, Ordering::SeqCst);
                    super::log("tap listening");
                    super::publish();
                }
            }
            if listening {
                // The tap's own source is a run loop source: without pumping the
                // run loop nothing is delivered, and the tap sits there deaf.
                // One second at a time keeps the loop free to re-check grants.
                unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, 1) };
            } else {
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }

    fn millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|span| span.as_millis() as u64)
            .unwrap_or(0)
    }

    fn holding(flags: u64) {
        HELD[0].store(flags & CONTROL != 0, Ordering::SeqCst);
        HELD[1].store(flags & OPTION != 0, Ordering::SeqCst);
        HELD[2].store(flags & SHIFT != 0, Ordering::SeqCst);
        HELD[3].store(flags & COMMAND != 0, Ordering::SeqCst);
        HELD[4].store(flags & SECONDARY_FN != 0, Ordering::SeqCst);
    }

    /// A key that only changes the modifiers is not a hotkey by itself.
    fn is_modifier(keycode: i64) -> bool {
        matches!(keycode, 54..=63)
    }

    unsafe extern "C" fn on_event(
        _proxy: *mut c_void,
        kind: u32,
        event: *mut c_void,
        _info: *mut c_void,
    ) -> *mut c_void {
        if kind == TAP_DISABLED_BY_TIMEOUT || kind == TAP_DISABLED_BY_INPUT {
            // macOS turns the tap off after a slow callback or a settings
            // change; without this line the hotkey dies for good.
            unsafe { CGEventTapEnable(TAP, true) };
            return event;
        }
        let keycode = unsafe { CGEventGetIntegerValueField(event, 9) };
        let flags = unsafe { CGEventGetFlags(event) };
        let capturing = super::CAPTURING.load(Ordering::SeqCst);

        if kind == FLAGS_CHANGED {
            let before = super::held_keys();
            holding(flags);
            let changed = before != super::held_keys();
            let fn_pressed = keycode == FN_KEYCODE && flags & SECONDARY_FN != 0;
            let fresh_press = fn_pressed && !FN_DOWN.swap(true, Ordering::SeqCst);
            if !fn_pressed {
                FN_DOWN.store(false, Ordering::SeqCst);
            }
            if fresh_press {
                let now = millis();
                let previous = LAST_FN.swap(now, Ordering::SeqCst);
                if previous != 0 && now.saturating_sub(previous) < DOUBLE_MS {
                    LAST_FN.store(0, Ordering::SeqCst);
                    if super::CAPTURING.load(Ordering::SeqCst) {
                        super::finish("FnFn");
                    } else {
                        let trigger = super::trigger();
                        if trigger.is_empty() || trigger.eq_ignore_ascii_case("FnFn") {
                            super::log("fn double tap");
                            super::toggle();
                        }
                    }
                    return event;
                }
            }
            if capturing && changed {
                super::publish();
            }
            return event;
        }

        if kind == KEY_UP {
            return event;
        }

        let modifiers = flags & (SHIFT | CONTROL | OPTION | COMMAND | SECONDARY_FN);

        if !capturing {
            // The person may bind any combination, not only Fn twice: the same
            // key code the Engine names for the settings is compared here.
            let trigger = super::trigger();
            if !trigger.is_empty()
                && !trigger.eq_ignore_ascii_case("FnFn")
                && modifiers != 0
                && !is_modifier(keycode)
            {
                std::thread::spawn(move || {
                    let Some(text) = accelerator(keycode, flags) else {
                        return;
                    };
                    if text.eq_ignore_ascii_case(&super::trigger()) {
                        super::log(&format!("hotkey {text}"));
                        super::toggle();
                    }
                });
            }
            return event;
        }
        if keycode == ESC_KEYCODE {
            super::log("capture cancelled");
            super::cancel("escape");
            return event;
        }
        if is_modifier(keycode) || modifiers == 0 {
            // A bare key types a letter; the person still holds the modifiers
            // of the combination they mean.
            super::publish();
            return event;
        }
        match accelerator(keycode, flags) {
            Some(text) => {
                super::log(&format!("captured {text}"));
                super::finish(&text);
            }
            None => super::publish(),
        }
        event
    }

    /// The Engine owns the keycode table — the same names it must parse back
    /// when dictation starts. Asking it keeps the two sides from drifting.
    fn accelerator(keycode: i64, flags: u64) -> Option<String> {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let answer = engine
            .rpc(
                "dictation.accelerator",
                json!({
                    "keyCode": keycode,
                    "cmd": flags & COMMAND != 0,
                    "ctrl": flags & CONTROL != 0,
                    "alt": flags & OPTION != 0,
                    "shift": flags & SHIFT != 0,
                    "fn": flags & SECONDARY_FN != 0,
                }),
            )
            .ok()?;
        answer
            .get("accelerator")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events: u64,
            callback: unsafe extern "C" fn(
                *mut c_void,
                u32,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void,
            user_info: *mut c_void,
        ) -> *mut c_void;
        fn CGEventTapEnable(tap: *mut c_void, enable: bool);
        fn CGEventGetFlags(event: *mut c_void) -> u64;
        fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
        fn CGPreflightListenEventAccess() -> bool;
        fn CGRequestListenEventAccess() -> bool;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: *mut c_void,
            order: i64,
        ) -> *mut c_void;
        fn CFRunLoopGetCurrent() -> *mut c_void;
        fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source: i32) -> i32;

        static kCFRunLoopCommonModes: *const c_void;
        static kCFRunLoopDefaultMode: *const c_void;
    }
}
