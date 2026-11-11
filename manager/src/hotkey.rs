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
static LAST_FN: AtomicU64 = AtomicU64::new(0);
/// Which wait for a combination is the current one, so an old timeout does
/// not cancel a new attempt.
static ATTEMPT: AtomicU64 = AtomicU64::new(0);
/// The combination the settings hold, as `dictation-config.json` has it.
static TRIGGER: Mutex<String> = Mutex::new(String::new());

pub fn start() {
    // The Manager keeps the tap, so the Engine's helper must go at once: it
    // carries its own signature, cannot see the key, and would fire a second
    // toggle if it ever did.
    release_engine_listener();
    #[cfg(target_os = "macos")]
    macos::start();
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

/// The combination the person set, read again while the app runs: the page
/// saves it through the Engine, and the tap has to follow within a second.
fn trigger() -> String {
    TRIGGER
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

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
fn toggle() {
    std::thread::spawn(|| {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let _ = engine.rpc("dictation.toggle", json!({}));
    });
}

/// The Engine's own listener would fire the same key a second time, and two
/// toggles are no toggle at all: the Manager keeps the tap and the Engine drops
/// its helper.
pub fn release_engine_listener() {
    std::thread::spawn(|| {
        let engine = crate::engine::Engine::new(crate::engine::data_dir());
        let _ = engine.rpc("dictation.release_hotkey", json!({}));
    });
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
            if !granted && asked.map_or(true, |at| at.elapsed() > Duration::from_secs(10)) {
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
