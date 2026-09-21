//! manager-gpui — GPUI Kosmos Manager. All state is owned by Kosmos Engine;
//! this shell only talks /v1/rpc + /v1/status over the local lock contract.
#![windows_subsystem = "windows"]

mod app;
mod devpkg;
mod engine;
mod fields;
mod modals;
mod render;
mod theme;
mod views;
mod widgets;
mod worker;

use gpui::{
    px, size, App, AppContext, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions,
};

use app::ManagerApp;

/// `MANAGER_GPUI_OFFSCREEN=1` parks the window far outside the desktop for
/// automated runs (same convention as agenda-gpui's AGENDA_OFFSCREEN).
fn window_bounds(cx: &mut App) -> Bounds<gpui::Pixels> {
    if std::env::var("MANAGER_GPUI_OFFSCREEN").is_ok() {
        gpui::bounds(
            gpui::point(px(-20000.), px(-20000.)),
            size(px(1280.), px(840.)),
        )
    } else {
        Bounds::centered(None, size(px(1280.), px(840.)), cx)
    }
}

fn main() {
    gpui::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        imago_gpui::theme::apply(cx);
        let bounds = window_bounds(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(SharedString::from("Kosmos Manager")),
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(12.), px(14.))),
                }),
                ..Default::default()
            },
            |window, cx| {
                let manager = cx.new(|cx| ManagerApp::new(window, cx));
                cx.new(|cx| gpui_component::Root::new(manager, window, cx))
            },
        )
        .unwrap();
        if std::env::var("MANAGER_GPUI_OFFSCREEN").is_err() {
            cx.activate(true);
        }
    });
}

#[allow(dead_code)]
fn _sig(_: &mut Window, _: &mut Context<ManagerApp>) {}
