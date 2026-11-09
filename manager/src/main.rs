//! Mundus Manager: a GPUI window whose pages are NSH (TSX), the same stack
//! Atlas runs — gpui-kit 0.8 on gpui-fast, `imago_nsh::AppView` hosting the
//! bundle, and `manager.bridge` connecting the pages to the Engine.

#![windows_subsystem = "windows"]

mod assets;
mod boot;
mod bridge;
mod chrome;
mod data;
mod engine;
mod identity;

use gpui::prelude::*;
use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};
#[cfg(target_os = "windows")]
use gpui_fast_backend as _;
use imago_nsh::{Bundle, NshApp};

const WINDOW_TITLE: &str = identity::PRODUCT_NAME;
const WINDOW_WIDTH: f32 = 1180.;
const WINDOW_HEIGHT: f32 = 820.;

/// The window's content: the NSH surface inside gpui-component's Root, with the
/// window's own caption buttons over it.
struct Shell {
    root: gpui::Entity<gpui_component::Root>,
}

impl gpui::Render for Shell {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        gpui::div()
            .size_full()
            .relative()
            .child(self.root.clone())
            .child(chrome::input())
            .children(chrome::controls(dark))
    }
}

fn main() {
    // The pages only mirror the Engine, so one has to answer before the window
    // opens. `--check-engine` runs that wait alone, for the installer smoke
    // test; `MUNDUS_SKIP_ENGINE_BOOT=1` leaves the lifetime to whoever started
    // this process (dev runs against an Engine already up).
    if std::env::var("MUNDUS_SKIP_ENGINE_BOOT").is_err() {
        if let Err(error) = boot::ensure_engine() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
    if std::env::args().any(|argument| argument == "--check-engine") {
        println!("Engine ready");
        return;
    }
    gpui::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.text_system()
            .add_fonts(assets::font_bytes())
            .expect("failed to load the design's fonts");
        // `MUNDUS_MANAGER_OFFSCREEN=1` parks the window off the desktop so a
        // soak run does not pop up over the user's work.
        let bounds = if std::env::var("MUNDUS_MANAGER_OFFSCREEN").is_ok() {
            gpui::bounds(
                gpui::point(px(-20000.), px(-20000.)),
                size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
            )
        } else {
            Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx)
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // Same as Atlas (KOS-368): a title at creation reaches AppKit's
            // `changeWindowsItem:` before the window is tracked, which leaves a
            // phantom Dock entry and turns the real window's close, minimize
            // and zoom into something other than the system's. `set_window_title`
            // below renames the entry AppKit added itself.
            titlebar: Some(gpui::TitlebarOptions {
                title: None,
                appears_transparent: true,
                ..Default::default()
            }),
            // The pages drag the window themselves (`desktop={{ windowDrag }}`)
            // from the page heading and the sidebar, so AppKit must not drag the
            // same strip a second time.
            app_owns_titlebar_drag: true,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            window.set_window_title(WINDOW_TITLE);
            let view = cx.new(|cx| {
                let view =
                    imago_nsh::AppView::new(nsh_platform_gpui::AppOptions::default(), window, cx);
                view.register_component(
                    "manager.bridge",
                    std::rc::Rc::new(|context| Box::new(bridge::ManagerBridge::new(context))),
                );
                view.on_error(|message| eprintln!("Manager nsh: {message}"));
                // Atlas passes its first snapshot as params; the Manager sends
                // the same state as a `state` event, so an empty map is enough —
                // but it must exist, because the pages read `host.params`.
                view.host().set_params(serde_json::Map::new());
                if let Ok(url) = std::env::var("MUNDUS_NSH_DEV_SERVER") {
                    view.connect_dev_server(url, cx);
                } else {
                    view.load(NshApp::Js(Bundle::source(
                        include_str!("../ui/bundle.js").to_owned(),
                    )));
                }
                view
            });
            // gpui-component widgets (the pages' inputs and menus) need a Root,
            // and the caption buttons sit above it: they belong to the window,
            // not to the pages (see `chrome`).
            let root = cx.new(|cx| gpui_component::Root::new(view, window, cx));
            cx.new(|_| Shell { root })
        })
        .expect("the Manager window opens");
        cx.activate(true);
    });
}
