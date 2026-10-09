//! manager-gpui — GPUI Mundus Manager. All state is owned by Mundus Engine;
//! this shell only talks /v1/rpc + /v1/status over the local lock contract.
#![windows_subsystem = "windows"]

mod app;
mod app_actions;
mod app_replies;
mod appearance_state;
mod assets;
mod async_fields;
mod boot;
mod button;
mod consent;
mod device_info;
mod devpkg;
mod fps;
mod logo_anim;
mod modals;
#[cfg(target_os = "macos")]
mod native_menu;
#[cfg(test)]
mod navigation_tests;
mod page_layout;
mod render;
mod theme;
mod toggle;
mod update_overlay;
mod views;
mod widgets;
mod worker;

use gpui::{px, size, App, AppContext, Bounds, Styled, WindowBounds, WindowOptions};

use app::ManagerApp;

/// `MANAGER_GPUI_OFFSCREEN=1` parks the window far outside the desktop for
/// automated runs (same convention as agenda-gpui's AGENDA_OFFSCREEN).
fn window_bounds(cx: &mut App) -> Bounds<gpui::Pixels> {
    window_bounds_for_test(cx)
}

pub(crate) fn window_bounds_for_test(cx: &mut App) -> Bounds<gpui::Pixels> {
    if std::env::var("MANAGER_GPUI_OFFSCREEN").is_ok() {
        gpui::bounds(
            gpui::point(px(-20000.), px(-20000.)),
            size(px(1280.), px(840.)),
        )
    } else {
        Bounds::centered(None, size(px(1280.), px(840.)), cx)
    }
}

const WINDOW_TITLE: &str = "Mundus";

pub(crate) fn manager_window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(window_bounds(cx))),
        titlebar: Some(gpui::TitlebarOptions {
            // KOS-368: no creation-time title. gpui's macOS backend routes
            // set_title through -[NSApplication changeWindowsItem:title:…],
            // which *adds* the window to the Dock/Window list when AppKit has
            // not tracked it yet; ordering the window in afterwards adds a
            // second, phantom entry (the two Dock windows Jack saw).
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(gpui::point(px(12.), px(14.))),
        }),
        ..Default::default()
    }
}

pub(crate) fn open_manager_window(cx: &mut App) {
    let options = manager_window_options(cx);
    cx.open_window(options, |window, cx| {
        // The platform window is already ordered in when this closure runs,
        // so set_title only renames the Dock/Window-list entry AppKit added
        // itself instead of inserting a phantom one.
        window.set_window_title(WINDOW_TITLE);
        let manager = cx.new(|cx| ManagerApp::new(window, cx));
        app_actions::register(cx, manager.downgrade());
        cx.new(|cx| gpui_component::Root::new(manager, window, cx).bg(gpui::rgba(0)))
    })
    .unwrap();
}

fn main() {
    if let Err(error) = boot::ensure_engine() {
        eprintln!("{error}");
        #[cfg(target_os = "macos")]
        if !std::env::args().any(|arg| arg == "--check-engine") {
            // Pass the message as argv, not interpolated AppleScript source.
            let _ = std::process::Command::new("/usr/bin/osascript")
                .args([
                    "-e",
                    concat!(
                        "on run argv\n",
                        "display alert \"Не удалось запустить Cortex\" ",
                        "message (item 1 of argv) as critical\nend run"
                    ),
                    &error,
                ])
                .status();
        }
        std::process::exit(1);
    }
    if std::env::args().any(|arg| arg == "--check-engine") {
        println!("Engine ready");
        return;
    }
    gpui::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            cx.text_system()
                .add_fonts(imago_gpui::assets::font_bytes())
                .expect("load Imago fonts");
            imago_gpui::theme::apply(cx);
            open_manager_window(cx);
            #[cfg(target_os = "macos")]
            native_menu::install(cx);
            if std::env::var("MANAGER_GPUI_OFFSCREEN").is_err() {
                cx.activate(true);
            }
        });
}

#[cfg(test)]
mod a11y_tests;
