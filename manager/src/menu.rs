//! The application menu macOS expects from a native app.
//!
//! `nsh-platform-gpui` builds that menu only for a *standalone* surface
//! (`chrome::render` → `set_native_menus`, gated on `standalone`). The Manager
//! embeds the NSH pages in its own gpui window, so the surface is not
//! standalone, and without a menu Cmd+Q is bound to nothing: the key reaches
//! the window and no item answers. The window's own menu is placed here.

use gpui::{actions, App, KeyBinding, Menu, MenuItem};

actions!(mundus, [QuitApp, CloseWindow, MinimizeWindow, HideApp]);

// The quit handler's subscription: dropping it would cancel the handler. The
// app runs on one thread, and a subscription cannot cross threads.
thread_local! {
    static KEPT: std::cell::RefCell<Vec<gpui::Subscription>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Places the menu bar and binds the shortcuts a person expects.
pub fn install(cx: &mut App) {
    let name = crate::identity::PRODUCT_NAME;
    cx.bind_keys([
        KeyBinding::new("cmd-q", QuitApp, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", MinimizeWindow, None),
        KeyBinding::new("cmd-h", HideApp, None),
    ]);
    cx.set_menus(vec![
        Menu {
            name: name.into(),
            items: vec![
                MenuItem::action(format!("Скрыть {name}"), HideApp),
                MenuItem::separator(),
                MenuItem::action(format!("Завершить {name}"), QuitApp),
            ],
            disabled: false,
        },
        Menu {
            name: "Окно".into(),
            items: vec![
                MenuItem::action("Свернуть", MinimizeWindow),
                MenuItem::action("Закрыть", CloseWindow),
            ],
            disabled: false,
        },
    ]);
    cx.on_action(|_: &QuitApp, cx: &mut App| cx.quit());
    // Whoever starts the quit — the menu item, AppKit's own terminate, a
    // window close — the Engine takes the hotkey back, so dictation keeps
    // working with only the menu bar icon left. The handler outlives this call,
    // so its subscription is kept rather than dropped.
    let subscription = cx.on_app_quit(|cx| {
        let task = cx
            .background_executor()
            .spawn(async { crate::handover::to_engine(crate::hotkey::listening()) });
        async move {
            task.await;
        }
    });
    KEPT.with(|kept| kept.borrow_mut().push(subscription));
    cx.on_action(|_: &HideApp, cx: &mut App| cx.hide());
    cx.on_action(|_: &MinimizeWindow, cx: &mut App| {
        if let Some(handle) = cx.active_window() {
            let _ = handle.update(cx, |_, window, _| window.minimize_window());
        }
    });
    cx.on_action(|_: &CloseWindow, cx: &mut App| {
        if let Some(handle) = cx.active_window() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    });
}
