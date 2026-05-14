// Phase 1.6: tray-icon UI layer.
//
// Tray icon живёт на собственном std::thread (event loop через `tao`), пока
// tokio runtime в `main.rs` обрабатывает WS server / async. Communication
// между ними через atomic `SHUTDOWN` flag — пользовательский клик "Выход" в
// tray menu выставляет флаг, main loop в kepler main.rs его проверяет и
// graceful shutdown'ится.
//
// Иконка резолвится по env `KEPLER_ICON_PATH` или ищется рядом с exe; в dev
// fallback используется `apps/kepler/icons/master.png`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

/// Global shutdown flag. Tray "Выход" outsets его в true; main loop в `main.rs`
/// проверяет периодически.
pub static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Phase 6 partial: счётчик нажатий Alt+Space. Future launcher window открывается
/// при этом сигнале — пока что только logged.
pub static LAUNCHER_REQUESTS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Спавнит tray на отдельной std-thread. Возвращает `None` если иконка не
/// найдена или tray не смог создаться (например, в headless среде или CI).
/// При None — main loop работает без tray-иконки.
pub fn spawn(tooltip_template: String) -> Option<JoinHandle<()>> {
    let icon_path = resolve_icon_path()?;
    let icon = match load_icon_via_image(&icon_path) {
        Some(i) => i,
        None => {
            eprintln!("[kepler.tray] failed to load icon from {icon_path:?}");
            return None;
        }
    };

    let handle = std::thread::Builder::new()
        .name("kepler-tray".to_string())
        .spawn(move || {
            run_event_loop(icon, tooltip_template);
        })
        .ok()?;

    Some(handle)
}

/// PNG → RGBA через `image` крейт → `Icon::from_rgba`. `Icon::from_path` на
/// Windows иногда падает с `CreateIconFromResourceEx` ошибкой для не-стандартных
/// PNG (RGBA с премультипл. альфой, очень большие размеры и т.д.); ручной decode
/// надёжнее.
fn load_icon_via_image(path: &std::path::Path) -> Option<Icon> {
    let bytes = std::fs::read(path)
        .map_err(|e| eprintln!("[kepler.tray] read icon failed: {e}"))
        .ok()?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| eprintln!("[kepler.tray] decode icon failed: {e}"))
        .ok()?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h)
        .map_err(|e| eprintln!("[kepler.tray] Icon::from_rgba failed: {e}"))
        .ok()
}

fn run_event_loop(icon: Icon, tooltip: String) {
    let mut builder = EventLoopBuilder::new();
    // На Windows tao event loop по умолчанию требует main thread; разрешаем
    // side-thread, потому что main thread у нас занят eframe launcher'ом.
    #[cfg(windows)]
    {
        builder.with_any_thread(true);
    }
    let event_loop = builder.build();

    let menu = Menu::new();
    let status_item = MenuItem::new("Kosmos Kepler", false, None);
    let exit_item = MenuItem::new("Выход", true, None);
    if let Err(e) = menu.append(&status_item) {
        eprintln!("[kepler.tray] menu append status failed: {e}");
        return;
    }
    if let Err(e) = menu.append(&tray_icon::menu::PredefinedMenuItem::separator()) {
        eprintln!("[kepler.tray] menu append separator failed: {e}");
        return;
    }
    if let Err(e) = menu.append(&exit_item) {
        eprintln!("[kepler.tray] menu append exit failed: {e}");
        return;
    }

    let _tray = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(&tooltip)
        .with_icon(icon)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[kepler.tray] tray build failed: {e}");
            return;
        }
    };

    let menu_channel = MenuEvent::receiver();
    let exit_id = exit_item.id().clone();

    // Global hotkey Ctrl+Shift+K (K = Kosmos/Kepler). Не конфликтует с Win-default'ами
    // (Alt+Space у Windows для context-menu окна, PowerToys Run тоже Alt+Space).
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    let launcher_hotkey = HotKey::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyK,
    );
    let launcher_hotkey_id = launcher_hotkey.id();
    if let Some(mgr) = hotkey_manager.as_ref() {
        match mgr.register(launcher_hotkey) {
            Ok(()) => eprintln!("[kepler.hotkey] Ctrl+Shift+K registered"),
            Err(e) => eprintln!("[kepler.hotkey] Ctrl+Shift+K register failed: {e}"),
        }
    } else {
        eprintln!("[kepler.hotkey] GlobalHotKeyManager init failed — hotkey disabled");
    }
    let hotkey_channel = GlobalHotKeyEvent::receiver();

    eprintln!("[kepler.tray] tray icon installed (right-click для menu)");

    // event_loop.run() — `!`, never returns.
    event_loop.run(move |_event, _target, control_flow| {
        // Полиwait — рассматриваем menu / hotkey events каждые 100ms.
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        while let Ok(event) = menu_channel.try_recv() {
            if event.id == exit_id {
                eprintln!("[kepler.tray] 'Выход' clicked — signalling shutdown");
                SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        while let Ok(event) = hotkey_channel.try_recv() {
            if event.id == launcher_hotkey_id
                && event.state == global_hotkey::HotKeyState::Pressed
            {
                let n = LAUNCHER_REQUESTS.fetch_add(1, Ordering::SeqCst) + 1;
                eprintln!("[kepler.hotkey] Ctrl+Shift+K pressed (#{n}) — open launcher");
            }
        }

        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn resolve_icon_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KEPLER_ICON_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for name in ["kepler.png", "kepler.ico"] {
                let p = parent.join(name);
                if p.exists() {
                    return Some(p);
                }
            }
            // Также проверим icons/ subdir рядом с exe.
            for name in ["icons/master.png", "icons/kepler.png"] {
                let p = parent.join(name);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    // Dev fallback (запуск через cargo run из repo root).
    for candidate in [
        "apps/kepler/icons/master.png",
        "kepler.png",
        "../../apps/kepler/icons/master.png",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    eprintln!("[kepler.tray] icon file not found (set KEPLER_ICON_PATH or place kepler.png near exe)");
    None
}

/// Convenience accessor для проверки shutdown flag из других модулей.
pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Reset для тестов / повторного использования. Production не вызывает.
#[allow(dead_code)]
pub fn reset_shutdown() {
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
}

// Unused type — оставлен для будущей signature расширения.
#[allow(dead_code)]
pub type TrayHandle = Arc<JoinHandle<()>>;
